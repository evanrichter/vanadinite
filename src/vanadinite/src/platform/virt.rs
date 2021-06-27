// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{csr::satp, mem::PHYSICAL_OFFSET};
use core::sync::atomic::Ordering;

pub const fn plic_max_priority() -> usize {
    7
}

pub fn current_plic_context() -> usize {
    plic_context_for(crate::HART_ID.get())
}

pub fn plic_context_for(hart: usize) -> usize {
    1 + 2 * hart
}

#[derive(Debug, Clone, Copy)]
pub enum ExitStatus {
    Pass,
    Reset,
    Fail(u16),
}

impl ExitStatus {
    fn magic(self) -> u32 {
        match self {
            ExitStatus::Pass => Finisher::Pass as u32,
            ExitStatus::Reset => Finisher::Reset as u32,
            ExitStatus::Fail(_) => Finisher::Fail as u32,
        }
    }

    fn to_u32(self) -> u32 {
        let ret_code = match self {
            ExitStatus::Pass | ExitStatus::Reset => 0,
            ExitStatus::Fail(n) => n as u32,
        };

        (ret_code << 16) | self.magic()
    }
}

#[repr(u32)]
enum Finisher {
    Fail = 0x3333,
    Pass = 0x5555,
    Reset = 0x7777,
}

/// So right about now is where I wish QEMU was better documented. Searching
/// through the code on Github for about 45 minutes resulted in the following
/// discovery:
///
/// To exit QEMU from inside it, we have to write to a special memory location
/// with a certain format. This is know for x86{_64} and ARM/AArch64 but I
/// couldn't find any resources on it for RISC-V.
///
/// It turns out that the `virt` board uses the same MMIO debug stuff as the
/// SiFive board, so you can subsequently find the information in that
/// header/implementation file pair at time of writing:
///
/// https://github.com/qemu/qemu/blob/57c98ea9acdcef5021f5671efa6475a5794a51c4/include/hw/misc/sifive_test.h
/// https://github.com/qemu/qemu/blob/57c98ea9acdcef5021f5671efa6475a5794a51c4/hw/misc/sifive_test.c
///
/// Which is created here for the `virt` board:
///
/// https://github.com/qemu/qemu/blob/57c98ea9acdcef5021f5671efa6475a5794a51c4/hw/riscv/virt.c#L379
///
/// So with all of this information we can gather that to exit QEMU we must:
///
///     1. Construct a 32-bit value to write
///         1a. The bottom 16-bits are the status code
///         1b. The top 16-bits are the exit code (this is ignored for Finisher::Pass which is always 0)
///     2. Write this value to VIRT_TEST (0x100000) + 0x000000
///     3. Pray we've actually exited, otherwise panic
///
/// Update 2020-10-14: QEMU changed the behavior to disallow writes larger than
/// 4 bytes and smaller than 2 bytes...
pub fn exit(exit_status: ExitStatus) -> ! {
    let virt_test: *mut u32 = match satp::read().mode {
        satp::SatpMode::Bare => 0x10_0000 as *mut u32,
        _ => (PHYSICAL_OFFSET.load(Ordering::Acquire) + 0x10_0000) as *mut u32,
    };

    unsafe {
        core::ptr::write_volatile(virt_test, exit_status.to_u32());
    }

    unreachable!()
}
