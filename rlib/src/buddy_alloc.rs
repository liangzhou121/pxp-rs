/*
The memory's arragement of this buddy system:

Memory:       [Start                                                               End]
free_lists: Vec<Vec<u32>>

block_num:    0_______________________________________________________________________
Memory:       |_______________________________________________________________________|
free_lists: Vec0:
              [block 0]

block_num:    0___________________________________1___________________________________
Memory:       |___________________________________|___________________________________|
free_lists: Vec1:
              [block 0, block 1]
Buddy:        block 0 <-> block 1

block_num:    0_________________1_________________2_________________3_________________
Memory:       |_________________|_________________|_________________|_________________|
free_lists: Vec2:
              [block 0, block 1, block 2, block 3]
Buddy:        block 0 <-> block 1, block 2 <-> block 3

block_num:    0________1________2________3________4________5________6________7________
Memory:       |________|________|________|________|________|________|________|________|
free_lists: Vec3:
              [block 0, block 1, block 2, block 3, block 4, block 5, block 6, block 7]
Buddy:        block 0 <-> block 1, block 2 <-> block 3, block 4 <-> block 5, block 6 <-> block 7
*/

use alloc::alloc::Layout;
use alloc::vec::Vec;
use core::cmp;
use core::fmt::Display;
use core::ptr::NonNull;
use spin::{Mutex, RwLock};

//const MAX_ALLOCATORS_NUM: usize = 32;

pub struct BuddyAllocatorManager {
    buddy_allocators: RwLock<Vec<Mutex<BuddyAllocator>>>,
}

impl BuddyAllocatorManager {
    pub const fn new() -> BuddyAllocatorManager {
        // Create an empty buddy allocator list. At this point we're still using the dumb page allocator.
        //let buddy_allocators = RwLock::new(Vec::with_capacity(MAX_ALLOCATORS_NUM));
        let buddy_allocators = RwLock::new(Vec::new());
        BuddyAllocatorManager { buddy_allocators }
    }

    unsafe fn add_memory_area(&self, start_addr: usize, end_addr: usize, block_size: u16) {
        // Add a new buddy allocator to the list with these specs.
        // As each one has some dynamic internal structures, we try to make it so that none of these
        // has to use itself when allocating these.
        let new_buddy_alloc = Mutex::new(BuddyAllocator::new(start_addr, end_addr, block_size));
        // On creation the buddy allocator constructor might lock the list of buddy allocators
        // due to the fact that it allocates memory for its internal structures (except for the very
        // first buddy allocator which still uses the previous, dumb allocator).
        // Therefore we first create it and then we lock the list in order to push the new
        // buddy allocator to the list.
        self.buddy_allocators.write().push(new_buddy_alloc);
    }

    /// Add a range of memory [start, end) to the heap
    /// block_size - the size of blocks on the leaf level
    pub unsafe fn init(&self, start: usize, size: usize, block_size: u16) {
        /*info!(
            "buddy alloc: add memory [start:{:x}, size:{:x}]",
            &start, &size
        );*/
        self.add_memory_area(start, start + size, block_size)
    }

    /// Alloc a range of memory from the buddy system satifying `layout` requirements
    pub fn alloc(&self, layout: Layout) -> Result<NonNull<u8>, ()> {
        // Loop through the list of buddy allocators until we can find one that can give us
        // the requested memory.
        self.buddy_allocators
            .read()
            .iter()
            .enumerate()
            .find_map(|(_i, allocator)| {
                // for each allocator
                allocator.try_lock().and_then(|mut allocator| {
                    allocator
                        .alloc(layout.size(), layout.align())
                        .map(|allocation| {
                            // try allocating until one succeeds and return this allocation
                            // info!(
                            //     " - BuddyAllocator #{} allocated {} bytes",
                            //     i,
                            //     layout.size()
                            // );
                            // info!("{}", *allocator);
                            allocation
                        })
                })
            })
            .map(|ptr| NonNull::new(ptr as *mut u8).unwrap())
            .ok_or_else(|| ())
    }

    /// Dealloc a range of memory from the buddy system
    pub fn dealloc(&self, ptr: NonNull<u8>, layout: Layout) {
        let addr = ptr.as_ptr() as usize;
        for (_i, allocator_mtx) in self.buddy_allocators.read().iter().enumerate() {
            // for each allocator
            if let Some(mut allocator) = allocator_mtx.try_lock() {
                // find the one whose memory range contains this address
                if allocator.contains(addr) {
                    // deallocate using this allocator!
                    allocator.dealloc(addr, layout.size(), layout.align());
                    // info!(
                    //     " - BuddyAllocator #{} de-allocated {} bytes",
                    //     i,
                    //     layout.size()
                    // );
                    // info!("{}", *allocator);
                    return;
                }
            }
        }
        // info!(
        //     "! Could not de-allocate virtual address: {} / Memory lost",
        //     virt_addr
        // );
    }

    pub fn fetch_memory_ranges(&self) -> Result<Vec<usize>, ()> {
        let mut ranges = Vec::new();
        for (_i, allocator_mtx) in self.buddy_allocators.read().iter().enumerate() {
            // for each allocator
            if let Some(allocator) = allocator_mtx.try_lock() {
                ranges.push(allocator.start_addr);
            }
        }
        Ok(ranges)
    }
}

struct BuddyAllocator {
    start_addr: usize,         // the first physical address that this struct manages
    end_addr: usize,           // one byte after the last physical address that this struct manages
    num_levels: u8,            // the number of non-leaf levels
    block_size: u16,           // the size of blocks on the leaf level
    free_lists: Vec<Vec<u32>>, // the list of free blocks on each level
}

impl BuddyAllocator {
    fn new(start_addr: usize, end_addr: usize, block_size: u16) -> BuddyAllocator {
        // number of levels excluding the leaf level
        let mut num_levels: u8 = 0;
        while ((block_size as usize) << num_levels as usize) < end_addr - start_addr {
            num_levels += 1;
        }
        // vector of free lists
        let mut free_lists: Vec<Vec<u32>> = Vec::with_capacity((num_levels + 1) as usize);
        // Initialize each free list with a small capacity (in order to use the current allocator
        // at least for the first few items and not the one that will be in use when we're actually
        // using this as the allocator as this might lead to this allocator using itself and locking)
        for _ in 0..(num_levels + 1) {
            free_lists.push(Vec::with_capacity(4));
        }
        // The top-most block is (the only) free for now!
        free_lists[0].push(0);
        // We need 1<<levels bits to store which blocks are split (so 1<<(levels-3) bytes)
        BuddyAllocator {
            start_addr,
            end_addr,
            num_levels,
            block_size,
            free_lists,
        }
    }

    fn contains(&self, addr: usize) -> bool {
        // whether a given physical address belongs to this allocator
        addr >= self.start_addr && addr < self.end_addr
    }

    fn max_size(&self) -> usize {
        // max size that can be supported by this buddy allocator
        (self.block_size as usize) << (self.num_levels as usize)
    }

    fn req_size_to_level(&self, size: usize) -> Option<usize> {
        // Find the level of this allocator than can accommodate the required memory size.
        let max_size = self.max_size();
        if size > max_size {
            // can't allocate more than the maximum size for this allocator!
            None
        } else {
            // find the largest block level that can support this size
            let mut next_level = 1;
            while (max_size >> next_level) >= size {
                next_level += 1;
            }
            // ...but not larger than the max level!
            let req_level = cmp::min(next_level - 1, self.num_levels as usize);
            Some(req_level)
        }
    }

    fn alloc(&mut self, size: usize, alignment: usize) -> Option<usize> {
        // We should always be aligned due to how the buddy allocator works
        // (everything will be aligned to block_size bytes).
        // If we need in some case that we are aligned to a greater size,
        // allocate a memory block of (alignment) bytes.
        let size = cmp::max(size, alignment);
        // find which level of this allocator can accommodate this amount of memory (if any)
        self.req_size_to_level(size).and_then(|req_level| {
            // We can accommodate it! Now to check if we actually have / can make a free block
            // or we're too full.
            self.get_free_block(req_level).map(|block| {
                // We got a free block!
                // get_free_block gives us the index of the block in the given level
                // so we need to find the size of each block in that level and multiply by the index
                // to get the offset of the memory that was allocated.
                let offset = block as usize * (self.max_size() >> req_level) as usize;
                // Add the base address of this buddy allocator's block and return
                self.start_addr + offset
            })
        })
    }

    fn dealloc(&mut self, addr: usize, size: usize, alignment: usize) {
        // As above, find which size was used for this allocation so that we can find the level
        // that gave us this memory block.
        let size = cmp::max(size, alignment);
        // find which level of this allocator was used for this memory request
        if let Some(req_level) = self.req_size_to_level(size) {
            // find size of each block at this level
            let level_block_size = self.max_size() >> req_level;
            // calculate which # block was just freed by using the start address and block size
            let block_num = ((addr - self.start_addr) / level_block_size) as u32;
            // push freed block to the free list so we can reuse it
            self.free_lists[req_level].push(block_num);
            // try merging buddy blocks now that we might have some to merge
            self.merge_buddies(req_level, block_num);
        }
    }

    fn merge_buddies(&mut self, level: usize, block_num: u32) {
        // toggle last bit to get buddy block
        let buddy_block = block_num ^ 1;
        // if buddy block in free list
        if let Some(buddy_idx) = self.free_lists[level]
            .iter()
            .position(|blk| *blk == buddy_block)
        {
            //info!("Merge the buddy blocks of [ {:?} : {:?} ]", block_num, buddy_block);
            // remove current block (in last place)
            self.free_lists[level].pop();
            // remove buddy block
            self.free_lists[level].remove(buddy_idx);
            // add free block to free list 1 level above
            self.free_lists[level - 1].push(block_num / 2);
            // repeat the process!
            self.merge_buddies(level - 1, block_num / 2)
        }
    }

    fn get_free_block(&mut self, level: usize) -> Option<u32> {
        // Get a block from the free list at this level or split a block above and
        // return one of the splitted blocks.
        self.free_lists[level]
            .pop()
            .or_else(|| self.split_level(level))
    }

    fn split_level(&mut self, level: usize) -> Option<u32> {
        // We reached the maximum level, we can't split anymore! We can't support this allocation.
        if level == 0 {
            None
        } else {
            self.get_free_block(level - 1).map(|block| {
                // Get a block from 1 level above us and split it.
                // We push the second of the splitted blocks to the current free list
                // and we return the other one as we now have a block for this allocation!
                self.free_lists[level].push(block * 2 + 1);
                block * 2
            })
        }
    }
}

impl Display for BuddyAllocator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut res = writeln!(
            f,
            "  Start: {:x?} / End: {:x?} / Levels: {} / Block size: {} / Max alloc: {}",
            self.start_addr,
            self.end_addr,
            self.num_levels + 1,
            self.block_size,
            (self.block_size as usize) << (self.num_levels as usize),
        );
        res = res.and_then(|_| write!(f, "  Free lists: "));
        for i in 0usize..(self.num_levels as usize + 1) {
            res = res.and_then(|_| write!(f, "{} in L{} / ", self.free_lists[i].len(), i));
        }
        res
    }
}
