// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(
    allocator_api,
    inline_const_pat,
    layout_for_ptr,
    never_type,
    ptr_metadata,
    slice_ptr_get,
    slice_ptr_len,
    try_trait_v2
)]
#![no_std]
#![allow(incomplete_features)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod capabilities;
pub mod error;
pub mod mem;
pub mod message;
pub mod syscalls;
pub mod task;
pub mod taskgroup;
