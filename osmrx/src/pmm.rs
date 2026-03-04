use crate::boot_info::{BootInfo, MemoryMapEntry, MemoryType};

pub const PAGE_SIZE: usize = 4096;
const MAX_REGIONS: usize = 256;

#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub struct PhysAddr(pub u64);

#[derive(Copy, Clone)]
pub struct PMRegion {
    pub start: PhysAddr,
    pub n_pages: u64,
}

static mut FREE_REGIONS: [PMRegion; MAX_REGIONS] = [PMRegion {
    start: PhysAddr(0),
    n_pages: 0,
}; MAX_REGIONS];
static mut FREE_COUNT: usize = 0;

static mut RESERVED_REGIONS: [PMRegion; MAX_REGIONS] = [PMRegion {
    start: PhysAddr(0),
    n_pages: 0,
}; MAX_REGIONS];
static mut RESERVED_COUNT: usize = 0;

static mut MAX_PHYS_ADDR: PhysAddr = PhysAddr(0);

fn end_addr(paddr: PhysAddr, n_pages: u64) -> PhysAddr {
    PhysAddr(paddr.0 + n_pages * PAGE_SIZE as u64)
}

fn regions_overlap(r1: PMRegion, r2: PMRegion) -> bool {
    let (mut a, mut b) = (r1, r2);
    if a.start.0 > b.start.0 {
        core::mem::swap(&mut a, &mut b);
    }
    let a_end = end_addr(a.start, a.n_pages);
    let b_end = end_addr(b.start, b.n_pages);
    a.start.0 < b_end.0 && b.start.0 < a_end.0
}

fn gap_between(prev: &MemoryMapEntry, curr: &MemoryMapEntry) -> Option<PMRegion> {
    let prev_end = end_addr(PhysAddr(prev.start), prev.n_pages);
    if curr.start <= prev_end.0 {
        return None;
    }
    let gap_bytes = curr.start - prev_end.0;
    let n_pages = gap_bytes / PAGE_SIZE as u64;
    if n_pages == 0 {
        return None;
    }
    Some(PMRegion {
        start: prev_end,
        n_pages,
    })
}

unsafe fn insert_free_region(start: PhysAddr, n_pages: u64) {
    if n_pages == 0 { return; }
    let new_start = start;
    let new_end = end_addr(start, n_pages);

    unsafe {
        let mut idx = 0;
        while idx < FREE_COUNT && FREE_REGIONS[idx].start.0 < new_start.0 {
            idx += 1;
        }

        if idx > 0 {
            let prev = &mut FREE_REGIONS[idx - 1];
            let prev_end = end_addr(prev.start, prev.n_pages);
            if prev_end.0 == new_start.0 {
                prev.n_pages += n_pages;
                if idx < FREE_COUNT {
                    let next = FREE_REGIONS[idx];
                    let prev_new_end = end_addr(prev.start, prev.n_pages);
                    if prev_new_end.0 == next.start.0 {
                        prev.n_pages += next.n_pages;
                        for j in idx + 1..FREE_COUNT {
                            FREE_REGIONS[j - 1] = FREE_REGIONS[j];
                        }
                        FREE_COUNT -= 1;
                    }
                }
                return;
            }
        }

        if idx < FREE_COUNT {
            let next_start = FREE_REGIONS[idx].start;
            if new_end.0 == next_start.0 {
                FREE_REGIONS[idx].start = new_start;
                FREE_REGIONS[idx].n_pages += n_pages;
                return;
            }
        }

        if FREE_COUNT >= MAX_REGIONS { return; }

        for j in (idx..FREE_COUNT).rev() {
            FREE_REGIONS[j + 1] = FREE_REGIONS[j];
        }
        FREE_REGIONS[idx] = PMRegion { start: new_start, n_pages };
        FREE_COUNT += 1;
    }
}

pub unsafe fn init(boot_info: &BootInfo) {
    unsafe {
        FREE_COUNT = 0;
        RESERVED_COUNT = 0;
        MAX_PHYS_ADDR = PhysAddr(0);

        let entries = core::slice::from_raw_parts(
            boot_info.physical_memory_map.entries,
            boot_info.physical_memory_map.len,
        );

        for (i, entry) in entries.iter().enumerate() {
            if entry.n_pages == 0 { continue; }
            if entry.mem_type == MemoryType::Free {
                let end = end_addr(PhysAddr(entry.start), entry.n_pages);
                if end.0 > MAX_PHYS_ADDR.0 { MAX_PHYS_ADDR = end; }
            }

            if entry.mem_type == MemoryType::Reserved && RESERVED_COUNT < MAX_REGIONS {
                RESERVED_REGIONS[RESERVED_COUNT] = PMRegion {
                    start: PhysAddr(entry.start),
                    n_pages: entry.n_pages,
                };
                RESERVED_COUNT += 1;
            }

            if i > 0 && RESERVED_COUNT < MAX_REGIONS {
                if let Some(gap) = gap_between(&entries[i - 1], entry) {
                    RESERVED_REGIONS[RESERVED_COUNT] = gap;
                    RESERVED_COUNT += 1;
                }
            }

            if entry.mem_type == MemoryType::Free {
                insert_free_region(PhysAddr(entry.start), entry.n_pages);
            }
        }
    }
}

pub unsafe fn alloc_pages(n: u64) -> Option<PhysAddr> {
    if n == 0 { return None; }
    unsafe {
        for i in 0..FREE_COUNT {
            let region = &mut FREE_REGIONS[i];
            if region.n_pages >= n {
                let addr = region.start;
                if region.n_pages == n {
                    for j in i + 1..FREE_COUNT {
                        FREE_REGIONS[j - 1] = FREE_REGIONS[j];
                    }
                    FREE_COUNT -= 1;
                } else {
                    region.start = PhysAddr(region.start.0 + n * PAGE_SIZE as u64);
                    region.n_pages -= n;
                }
                return Some(addr);
            }
        }
    }
    None
}

pub unsafe fn total_free_pages() -> u64 {
    let mut total = 0;
    unsafe {
        for i in 0..FREE_COUNT {
            total += FREE_REGIONS[i].n_pages;
        }
    }
    total
}

pub unsafe fn free_pages(addr: PhysAddr, n: u64) {
    assert!(addr.0 % PAGE_SIZE as u64 == 0, "unaligned physical address");
    unsafe {
        let region_end = end_addr(addr, n);
        if region_end.0 > MAX_PHYS_ADDR.0 { loop { core::hint::spin_loop(); } }

        let request = PMRegion { start: addr, n_pages: n };
        for i in 0..RESERVED_COUNT {
            if RESERVED_REGIONS[i].n_pages == 0 { continue; }
            if regions_overlap(RESERVED_REGIONS[i], request) {
                loop { core::hint::spin_loop(); }
            }
        }
        insert_free_region(addr, n);
    }
}
