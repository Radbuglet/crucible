use fastrand::Rng;

use crate::mem::array::vec_from_fn;

// === Hashing === //

const DEFAULT_LAMBDA: usize = 5;

fn new_rng() -> Rng {
	Rng::with_seed(0xBADF00D)
}

fn scramble(state: u64) -> u64 {
	Rng::with_seed(state).u64(..)
}

fn randomize(key: u32, hash: u32) -> u64 {
	scramble(((key as u64) << 32) + hash as u64)
}

fn randomize_idx(key: u32, hash: u32, len: usize) -> usize {
	debug_assert!(len.is_power_of_two());
	(randomize(key, hash) & (len as u64 - 1)) as usize
}

// === Phf === //

#[derive(Debug, Clone)]
pub struct Phf {
	main_key: u32,
	slot_count: usize,
	bucket_keys: Vec<u32>,
}

impl Phf {
	pub fn new<S, SI>(elems: S) -> (Self, Vec<usize>)
	where
		S: Clone + IntoIterator<IntoIter = SI>,
		SI: ExactSizeIterator<Item = u32>,
	{
		// TODO: Check for overflows.

		#[derive(Debug)]
		struct Bucket {
			// The actual index of the bucket in the `bucket_keys` list.
			//
			// This is necessary because we sort the `buckets` array by their relative size to speed
			// up hashing.
			index: usize,

			// The list of key indices and their hashes stored in this bucket.
			keys: Vec<(usize, u32)>,
		}

		// Define generator state
		let rng = new_rng();
		let elem_count = elems.clone().into_iter().len();
		let slot_count = elem_count.next_power_of_two();
		let bucket_count = ((elem_count + DEFAULT_LAMBDA - 1) / DEFAULT_LAMBDA).next_power_of_two();
		let max_bucket_key = slot_count
			.checked_mul(slot_count)
			.and_then(|value| u32::try_from(value).ok())
			.unwrap_or(u32::MAX);

		let mut buckets = (0..bucket_count)
			.map(|index| Bucket {
				index,
				keys: Vec::new(),
			})
			.collect::<Vec<_>>();

		let mut bucket_keys = vec_from_fn(|| 0u32, bucket_count);
		let mut slot_to_index = vec_from_fn(|| usize::MAX, slot_count);
		let mut slot_gens = vec_from_fn(|| 0u64, slot_count);
		let mut curr_gen = 0u64;

		// Try main keys
		'finding_main_key: loop {
			let main_key = rng.u32(..);

			// Hash elements into buckets
			for (i, bucket) in buckets.iter_mut().enumerate() {
				bucket.index = i;
				bucket.keys.clear();
			}

			for (i, hash) in elems.clone().into_iter().enumerate() {
				buckets[randomize_idx(main_key, hash, bucket_count)]
					.keys
					.push((i, hash));
			}

			// Sort buckets by increasing size.
			buckets.sort_by(|a, b| a.keys.len().cmp(&b.keys.len()).reverse());

			// Try a bunch of bucket keys
			for slot in &mut slot_to_index {
				*slot = usize::MAX;
			}

			'setting_buckets: for bucket in &buckets {
				// Try to find an appropriate key
				'finding_key: for bucket_key in 0..=max_bucket_key {
					curr_gen += 1;

					// Try to slot things in using the key.
					for &(_, elem_hash) in &bucket.keys {
						let slot_idx = randomize_idx(bucket_key, elem_hash, slot_count);

						if curr_gen != slot_gens[slot_idx] && slot_to_index[slot_idx] == usize::MAX
						{
							slot_gens[slot_idx] = curr_gen;
						} else {
							continue 'finding_key;
						}
					}

					// We succeeded!
					bucket_keys[bucket.index] = bucket_key;

					for &(elem_idx, elem_hash) in &bucket.keys {
						let slot_idx = randomize_idx(bucket_key, elem_hash, slot_count);
						slot_to_index[slot_idx] = elem_idx;
					}

					continue 'setting_buckets;
				}

				// Otherwise, try with a new main key.
				continue 'finding_main_key;
			}

			// Each bucket has been properly set up, break.
			log::trace!("Generated PHF in {} bucket rehashe(s).", curr_gen);
			break (
				Self {
					slot_count,
					bucket_keys,
					main_key,
				},
				slot_to_index,
			);
		}
	}

	pub fn find_slot(&self, hash: u32) -> usize {
		assert!(!self.bucket_keys.is_empty());

		let bucket_index = randomize_idx(self.main_key, hash, self.bucket_keys.len());
		let bucket_key = self.bucket_keys[bucket_index];
		let entry_index = randomize_idx(bucket_key, hash, self.slot_count);
		entry_index
	}
}

// === Tests === //

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn phf_gen() {
		fastrand::seed(0xDEADBEEF);

		// Collect a bunch of hashes
		let mut elem_hashes = (0..100).map(|_| fastrand::u32(..)).collect::<Vec<_>>();
		elem_hashes.sort_by(|a, b| a.cmp(&b));

		// Remove duplicates
		{
			let mut prev = None;
			elem_hashes.retain_mut(|a| prev.replace(*a) != Some(*a));
		}

		let (phf, slot_to_idx) = Phf::new(elem_hashes.iter().copied());

		for (i, &hash) in elem_hashes.iter().enumerate() {
			assert_eq!(slot_to_idx[phf.find_slot(hash)], i);
		}
	}
}
