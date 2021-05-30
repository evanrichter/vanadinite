// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::VirtualAddress;
use crate::mem::region::MemoryRegion;
use alloc::collections::BTreeMap;
use core::ops::Range;

// TODO: probably could split this up slightly more and represent the
// {un}occupied regions as different types?
/// A region of memory allocated to a task
#[derive(Debug)]
pub struct AddressRegion {
    /// The underlying [`MemoryRegion`], which may or may not be backed by
    /// physical memory. `None` represents an unoccupied region.
    pub region: Option<MemoryRegion>,
    /// The region span
    pub span: Range<VirtualAddress>,
    /// The type of memory contained in the region, used for debugging purposes
    pub kind: AddressRegionKind,
}

impl AddressRegion {
    pub fn is_unoccupied(&self) -> bool {
        self.region.is_none()
    }
}

/// Describes what type of memory the address region contains
#[derive(Debug, Clone, Copy)]
pub enum AddressRegionKind {
    Channel,
    Data,
    Guard,
    ReadOnly,
    Stack,
    Text,
    Tls,
    Unoccupied,
    UserAllocated,
}

/// Represents the userspace address space and allows for allocating and
/// deallocating regions of the address space
pub struct AddressMap {
    map: BTreeMap<VirtualAddress, AddressRegion>,
}

impl AddressMap {
    /// Create a new [`AddressMap`]
    pub fn new() -> Self {
        let complete_range = VirtualAddress::userspace_range();
        let mut map = BTreeMap::new();
        map.insert(
            complete_range.end,
            AddressRegion { region: None, span: complete_range, kind: AddressRegionKind::Unoccupied },
        );

        Self { map }
    }

    /// Allocate a new virtual memory region backed by the given
    /// [`MemoryRegion`] at the given range. Returns `Err(())` if the region is
    /// already occupied.
    pub fn alloc(
        &mut self,
        subrange: Range<VirtualAddress>,
        backing: MemoryRegion,
        kind: AddressRegionKind,
    ) -> Result<(), ()> {
        let key = match self.map.range(subrange.end..).next() {
            Some((_, range))
                if range.span.start > subrange.start || range.span.end < subrange.end || range.region.is_some() =>
            {
                return Err(());
            }
            None => return Err(()),
            Some((key, _)) => *key,
        };

        let mut old_range = self.map.remove(&key).unwrap();

        match (old_range.span.start == subrange.start, old_range.span.end == subrange.end) {
            // Chop off the start
            (true, false) => {
                old_range.span = subrange.end..old_range.span.end;
                self.map.insert(old_range.span.end, old_range);
                self.map.insert(subrange.end, AddressRegion { region: Some(backing), span: subrange, kind });
            }
            // Chop off the end
            (false, true) => {
                old_range.span = old_range.span.start..subrange.start;
                self.map.insert(old_range.span.end, old_range);
                self.map.insert(subrange.end, AddressRegion { region: Some(backing), span: subrange, kind });
            }
            // its the whole ass range
            (true, true) => {
                self.map.insert(subrange.end, AddressRegion { region: Some(backing), span: subrange, kind });
            }
            // its a true subrange, need to splice out an generate 3 new ranges
            (false, false) => {
                let before = AddressRegion {
                    region: None,
                    span: old_range.span.start..subrange.start,
                    kind: AddressRegionKind::Unoccupied,
                };
                let active = AddressRegion { region: Some(backing), span: subrange.clone(), kind };
                let after = AddressRegion {
                    region: None,
                    span: subrange.end..old_range.span.end,
                    kind: AddressRegionKind::Unoccupied,
                };

                self.map.insert(before.span.end, before);
                self.map.insert(active.span.end, active);
                self.map.insert(after.span.end, after);
            }
        }

        Ok(())
    }

    /// Free the given range, returning the backing [`MemoryRegion`] or an
    /// `Err(())` if the range wasn't occupied
    pub fn free(&mut self, range: Range<VirtualAddress>) -> Result<MemoryRegion, ()> {
        match self.map.range(range.end..).next() {
            Some((_, curr_range))
                if curr_range.span.start != range.start
                    || curr_range.span.end != range.end
                    || curr_range.region.is_none() =>
            {
                return Err(());
            }
            None => return Err(()),
            _ => {}
        }

        let mut range = self.map.remove(&range.end).unwrap();

        // Coalesce free regions around into a single region
        while let Some((_, AddressRegion { region: None, .. })) = self.map.range(range.span.start..).next() {
            let start = self.map.remove(&range.span.start).unwrap().span.start;
            range.span.start = start;
        }

        while let Some((&key, AddressRegion { region: None, .. })) = self.map.range(range.span.end.offset(1)..).next() {
            let end = self.map.remove(&key).unwrap().span.end;
            range.span.end = end;
        }

        let ret = range.region.take().unwrap();

        self.map.insert(range.span.end, range);

        Ok(ret)
    }

    /// Find the region containing the given [`VirtualAddress`]
    pub fn find(&self, address: VirtualAddress) -> Option<&AddressRegion> {
        self.map.range(address..).next().map(|(_, r)| r)
    }

    /// Returns the unoccupied regions in the address space
    pub fn unoccupied_regions(&self) -> impl Iterator<Item = &AddressRegion> {
        self.map.values().filter(|v| v.region.is_none())
    }

    /// Returns the occupied regions in the address space
    pub fn occupied_regions(&self) -> impl Iterator<Item = &AddressRegion> {
        self.map.values().filter(|v| v.region.is_some())
    }
}

impl core::fmt::Debug for AddressMap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match f.alternate() {
            true => {
                for region in self.occupied_regions() {
                    writeln!(
                        f,
                        "[{:?}] {:#p}..{:#p}: {:?}",
                        region.region.as_ref().unwrap().page_size(),
                        region.span.start,
                        region.span.end,
                        region.kind,
                    )?;
                }

                Ok(())
            }
            false => f.debug_struct("AddressMap").field("map", &self.map).finish(),
        }
    }
}
