use std::mem::MaybeUninit;
use std::num::NonZeroU64 as Key;

pub struct PerfectMap<T, const SZ: usize> {
    buckets: [(u64, MaybeUninit<T>); SZ],
    mul: u64,
}

impl<T: Copy, const SZ: usize> PerfectMap<T, { SZ }> {
    pub const fn new(entries: &[(Key, T)]) -> Self {
        // Validate entries
        // TODO

        // Build table
        #[derive(Copy, Clone)]
        struct VirtualBucket {
            mul: u64,
            entry_idx: usize,
        }

        let mut mul = 0;
        let mut virtual_table = [VirtualBucket { mul, entry_idx: 0 }; SZ];

        'gen: loop {
            mul += 1;

            let mut entry_idx = 0;
            while entry_idx < entries.len() {
                let bucket_idx = Self::get_index(entries[entry_idx].0.get(), mul);
                let bucket = &mut virtual_table[bucket_idx];
                if bucket.mul == mul {
                    continue 'gen;
                }

                bucket.entry_idx = entry_idx;
                bucket.mul = mul;
                entry_idx += 1;
            }

            break;
        }

        let mut buckets = [(0, MaybeUninit::uninit()); SZ];
        let mut bucket_idx = 0;
        while bucket_idx < SZ {
            let bucket = &virtual_table[bucket_idx];

            if bucket.mul == mul {
                let (id, meta) = entries[bucket.entry_idx];
                buckets[bucket_idx] = (id.get(), MaybeUninit::new(meta));
            }

            bucket_idx += 1;
        }

        Self { buckets, mul }
    }
}

impl<T, const SZ: usize> PerfectMap<T, { SZ }> {
    const fn get_index(id: u64, mul: u64) -> usize {
        ((id * mul) % SZ as u64) as usize
    }

    pub const fn get(&self, id: Key) -> Option<&T> {
        let id = id.get();
        let (bucket_id, meta) = &self.buckets[Self::get_index(id, self.mul)];

        if *bucket_id == id {
            Some (unsafe { meta.assume_init_ref() })
        } else {
            None
        }
    }

    pub const fn get_mut(&mut self, id: Key) -> Option<&mut T> {
        let id = id.get();
        let (bucket_id, meta) = &mut self.buckets[Self::get_index(id, self.mul)];

        if *bucket_id == id {
            Some (unsafe { meta.assume_init_mut() })
        } else {
            None
        }
    }
}
