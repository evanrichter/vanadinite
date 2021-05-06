// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    csr::sstatus::TemporaryUserMemoryAccess,
    io::ConsoleDevice,
    mem::paging::{flags, VirtualAddress},
    task::Task,
    task::TaskState,
};
use libvanadinite::{syscalls::print::PrintErr, KResult};

pub fn print(current_task: &mut Task, virt: VirtualAddress, len: usize, res_out: VirtualAddress) {
    log::debug!("Attempting to print memory at {:#p} (len={})", virt, len);

    let (valid_memory, valid_res) = {
        let mm = &current_task.memory_manager;

        let flags_start = mm.page_flags(virt);
        let flags_end = mm.page_flags(virt.offset(len));
        let flags_res_out = mm.page_flags(res_out);

        (
            flags_start.zip(flags_end).map(|(fs, fe)| fs & flags::READ && fe & flags::READ).unwrap_or_default(),
            flags_res_out.map(|f| f & flags::WRITE).unwrap_or_default(),
        )
    };

    if !valid_res {
        log::error!("Invalid memory for print, killing");
        current_task.state = TaskState::Dead;
        return;
    }

    let _guard = TemporaryUserMemoryAccess::new();
    let res_out: *mut KResult<(), PrintErr> = res_out.as_mut_ptr().cast();

    if virt.is_kernel_region() {
        log::error!("Process tried to get us to read from our own memory >:(");
        unsafe { *res_out = KResult::Err(PrintErr::NoAccess) };
        return;
    } else if !valid_memory {
        log::error!("Process tried to get us to read from unmapped memory >:(");
        unsafe { *res_out = KResult::Err(PrintErr::NoAccess) };
        return;
    }

    let mut console = crate::io::CONSOLE.lock();
    let bytes = unsafe { core::slice::from_raw_parts(virt.as_ptr(), len) };
    for byte in bytes {
        console.write(*byte);
    }

    unsafe { *res_out = KResult::Ok(()) };
}
