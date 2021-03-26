// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    interrupts::InterruptDisabler,
    mem::phys2virt,
    mem::{
        paging::{EntryKind, PageSize, PhysicalAddress, Sv39PageTable, VirtualAddress},
        phys::PhysicalMemoryAllocator,
        sfence,
    },
    PHYSICAL_MEMORY_ALLOCATOR,
};

use super::Permissions;

#[derive(Debug)]
pub struct PageTableManager(*mut Sv39PageTable);

impl PageTableManager {
    pub fn new(table: *mut Sv39PageTable) -> Self {
        Self(table)
    }

    pub fn alloc_virtual_range(&mut self, start: VirtualAddress, size: usize, perms: Permissions) {
        assert_eq!(size % 4096, 0, "bad map range size: {}", size);

        for idx in 0..size / 4096 {
            self.alloc_virtual(start.offset(idx * 4096), perms);
        }
    }

    pub fn alloc_virtual(&mut self, map_to: VirtualAddress, perms: Permissions) {
        let _disabler = InterruptDisabler::new();
        let phys = Self::new_phys_page();

        log::debug!("PageTableManager::map_page: mapping {:#p} to {:#p}", phys, map_to);
        unsafe { &mut *self.0 }.map(phys, map_to, PageSize::Kilopage, perms);

        sfence(Some(map_to), None);
    }

    pub fn alloc_virtual_range_with_data(
        &mut self,
        start: VirtualAddress,
        size: usize,
        perms: Permissions,
        data: &[u8],
    ) {
        assert_eq!(size % 4096, 0, "bad map range size: {}", size);

        for (idx, data) in (0..size / 4096).zip(data.chunks(4096)) {
            self.alloc_virtual_with_data(start.offset(idx * 4096), perms, data);
        }
    }

    pub fn alloc_virtual_with_data(&mut self, map_to: VirtualAddress, perms: Permissions, data: &[u8]) {
        let _disabler = InterruptDisabler::new();
        let phys = Self::new_phys_page();

        log::debug!("PageTableManager::map_page: mapping {:#p} to {:#p}", phys, map_to);
        unsafe { &mut *self.0 }.map(phys, map_to, PageSize::Kilopage, perms);

        let ptr = phys2virt(phys).as_mut_ptr();

        for (i, byte) in data.iter().copied().enumerate() {
            unsafe { *ptr.add(i) = byte };
        }

        sfence(Some(map_to), None);
    }

    pub fn map_direct(
        &mut self,
        map_from: PhysicalAddress,
        map_to: VirtualAddress,
        size: PageSize,
        perms: Permissions,
    ) {
        let _disabler = InterruptDisabler::new();
        unsafe { &mut *self.0 }.map(map_from, map_to, size, perms);

        sfence(Some(map_to), None);
    }

    pub fn modify_page_permissions(&mut self, virt: VirtualAddress, new_permissions: Permissions) {
        if let Some((entry, _)) = unsafe { &mut *self.0 }.entry_mut(virt) {
            entry.set_permissions(new_permissions);
        }
    }

    pub fn resolve(&self, virt: VirtualAddress) -> Option<PhysicalAddress> {
        unsafe { &*self.0 }.translate(virt)
    }

    pub fn table(&self) -> *mut Sv39PageTable {
        self.0
    }

    pub fn copy_kernel_pages(&mut self) {
        let current = unsafe { &*Sv39PageTable::current() };

        let start_idx = VirtualAddress::new(0xFFFFFFC000000000).vpns()[2];
        for i in start_idx..512 {
            unsafe { &mut *self.0 }.entries[i] = current.entries[i];
        }
    }

    pub fn is_valid_readable(&self, virt: VirtualAddress) -> bool {
        match unsafe { &*self.0 }.entry(virt) {
            Some((entry, _)) => entry.is_readable(),
            None => false,
        }
    }

    pub fn is_valid_writable(&self, virt: VirtualAddress) -> bool {
        match unsafe { &*self.0 }.entry(virt) {
            Some((entry, _)) => entry.is_writable(),
            None => false,
        }
    }

    pub fn debug_print(&self) -> PageTableDebugPrint {
        PageTableDebugPrint(self.0)
    }

    fn new_phys_page() -> PhysicalAddress {
        unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc().expect("we oom, rip") }.as_phys_address()
    }
}

pub struct PageTableDebugPrint(*mut Sv39PageTable);

impl core::fmt::Debug for PageTableDebugPrint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let end_n = VirtualAddress::new(0xFFFFFFC000000000).vpns()[2];
        writeln!(f, "\n")?;
        for gib_entry_i in 0..end_n {
            let gib_entry = &unsafe { &*self.0 }.entries[gib_entry_i];
            let next_table = match gib_entry.kind() {
                EntryKind::Leaf => {
                    writeln!(
                        f,
                        "[G] {:#p} => {:#p}",
                        VirtualAddress::new(gib_entry_i << 30),
                        gib_entry.ppn().unwrap()
                    )?;
                    continue;
                }
                EntryKind::NotValid => continue,
                EntryKind::Branch(phys) => unsafe { &*phys2virt(phys).as_mut_ptr().cast::<Sv39PageTable>() },
            };

            for mib_entry_i in 0..512 {
                let mib_entry = &next_table.entries[mib_entry_i];
                let next_table = match mib_entry.kind() {
                    EntryKind::Leaf => {
                        writeln!(
                            f,
                            "[M] {:#p} => {:#p}",
                            VirtualAddress::new((gib_entry_i << 30) | (mib_entry_i << 21)),
                            mib_entry.ppn().unwrap()
                        )?;
                        continue;
                    }
                    EntryKind::NotValid => continue,
                    EntryKind::Branch(phys) => unsafe { &*phys2virt(phys).as_mut_ptr().cast::<Sv39PageTable>() },
                };

                for kib_entry_i in 0..512 {
                    let kib_entry = &next_table.entries[kib_entry_i];
                    match kib_entry.kind() {
                        EntryKind::Leaf => {
                            writeln!(
                                f,
                                "[K] {:#p} => {:#p}",
                                VirtualAddress::new((gib_entry_i << 30) | (mib_entry_i << 21) | (kib_entry_i << 12)),
                                kib_entry.ppn().unwrap()
                            )?;
                            continue;
                        }
                        EntryKind::NotValid => continue,
                        EntryKind::Branch(_) => unreachable!("A KiB PTE was marked as a branch?"),
                    }
                }
            }
        }
        writeln!(f, "\n")?;

        Ok(())
    }
}
