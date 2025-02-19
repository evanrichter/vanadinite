// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CapabilityPtr(usize);

impl CapabilityPtr {
    pub fn new(n: usize) -> Self {
        Self(n)
    }

    pub fn value(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CapabilityRights(usize);

impl CapabilityRights {
    pub const READ: Self = Self(1);
    pub const WRITE: Self = Self(2);
    pub const EXECUTE: Self = Self(4);
    pub const GRANT: Self = Self(8);
}

impl CapabilityRights {
    pub fn new(value: usize) -> Self {
        Self(value & 0xF)
    }

    pub fn is_superset(self, other: Self) -> bool {
        (self.0 | !other.0) == usize::MAX
    }

    pub fn value(self) -> usize {
        self.0
    }
}

impl core::ops::BitOr for CapabilityRights {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        CapabilityRights(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for CapabilityRights {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = CapabilityRights(self.0 | rhs.0);
    }
}

impl core::ops::BitAnd for CapabilityRights {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Capability {
    pub cptr: CapabilityPtr,
    pub rights: CapabilityRights,
}

impl Capability {
    pub fn new(cptr: CapabilityPtr, rights: CapabilityRights) -> Self {
        Self { cptr, rights }
    }
}

impl Default for Capability {
    fn default() -> Self {
        Self { cptr: CapabilityPtr::new(0), rights: CapabilityRights::new(0) }
    }
}
