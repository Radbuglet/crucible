use crucible_common::math::num::OrdF32;
use crucible_common::math::range::{unwrap_or_unbounded, AnyRange};
use std::collections::Bound;
use std::fmt::{Debug, Display};

#[derive(Debug, Clone)]
pub struct FeatureList {
	/// A list of features comprising the overall feature list.
	entries: Vec<FeatureEntry>,

	/// The number of mandatory features that have been met.
	met_mandatory: usize,

	/// The number of optional features that have been met.
	met_optional: usize,

	/// The number of features that are mandatory (counted shallowly). All the other features in the
	/// `entries` vector are optional.
	total_mandatory: usize,

	/// The cumulative score of the feature list.
	score: OrdF32,

	/// The range of possible scores.
	score_range: AnyRange<f32>,
}

#[derive(Debug, Clone)]
pub struct FeatureDescriptor<N = String, D = String> {
	/// The short name of the feature.
	pub name: N,

	/// A longer description of why the feature is useful.
	pub description: D,
}

impl<N: Display, D: Display> FeatureDescriptor<N, D> {
	pub fn to_strings(&self) -> FeatureDescriptor {
		FeatureDescriptor {
			name: self.name.to_string(),
			description: self.description.to_string(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct FeatureEntry {
	/// A description of the feature.
	pub desc: FeatureDescriptor,

	/// Whether the feature is mandatory or optional.
	pub mandatory: bool,

	/// The score of the given feature entry.
	pub score: FeatureScore,

	/// A logical sub-table justifying the score on this feature. This is not automatically taken
	/// into consideration when computing this feature's score.
	pub sub: Option<FeatureList>,
}

#[derive(Debug, Clone)]
pub enum FeatureScore {
	/// The feature is supported and does not display an explicit score.
	BinaryPass,

	/// The feature is not supported at all, with no explicit score given.
	BinaryFail {
		/// A string listing the reasons for which the feature is not supported and ways to allow it
		/// to be supported. A sub-feature-list (specified by [FeatureList::sub]`) may be more
		/// effective for this if a given logical feature requires several sub-features.
		reason: String,
	},

	/// The feature is supported, albeit potentially only partially.
	ScorePass {
		/// A floating-point (potentially negative) score describing the overall weight of this
		/// feature. A percentage will be computed and rendered instead of the raw score if a
		/// `score_range` is provided.
		score: OrdF32,

		/// The range of scores this feature can take.
		score_range: AnyRange<f32>,

		/// A string listing the reasons for which the feature may or may not be performing optimally,
		/// and ways to allow it to be supported. A sub-feature-list (specified by [FeatureList::sub]`)
		/// may be more effective for this if a given logical feature requires several sub-features.
		reason: String,
	},

	/// This feature is not applicable to this given device. This is considered as having met the
	/// conditions but shows up differently in the troubleshoot menu.
	NotApplicable { reason: String },
}

impl FeatureScore {
	pub fn is_met(&self) -> bool {
		!matches!(self, FeatureScore::BinaryFail { .. })
	}
}

impl Default for FeatureList {
	fn default() -> Self {
		Self {
			entries: Vec::new(),
			met_mandatory: 0,
			met_optional: 0,
			score: OrdF32::ZERO,
			total_mandatory: 0,
			score_range: AnyRange::new(0.0..=0.0),
		}
	}
}

impl FeatureList {
	pub fn push_raw(&mut self, entry: FeatureEntry) {
		// Count met percentages
		if entry.mandatory {
			if entry.score.is_met() {
				self.met_mandatory += 1;
			}

			self.total_mandatory += 1;
		} else {
			if entry.score.is_met() {
				self.met_optional += 1;
			}

			// `total_optional` is implicitly updated when we push another entry because
			// `total_optional = self.entries.len() - total_mandatory`.
		}

		// Update score ranges
		if let FeatureScore::ScorePass {
			score, score_range, ..
		} = &entry.score
		{
			self.score += *score;

			// Low
			if let (Some(total_low), Some(this_low)) = (
				// The difference between the two is f32::EPSILON. Let's just treat them as being
				// pretty much the same.
				//
				// - Fanatics of Infinitesimals
				unwrap_or_unbounded(self.score_range.start),
				unwrap_or_unbounded(score_range.start),
			) {
				self.score_range.start = Bound::Included(total_low + this_low);
			} else {
				// Anything can be anything.
				self.score_range.start = Bound::Unbounded;
			}

			// High
			if let (Some(total_high), Some(this_high)) = (
				unwrap_or_unbounded(self.score_range.end),
				unwrap_or_unbounded(score_range.end),
			) {
				self.score_range.end = Bound::Included(total_high + this_high);
			} else {
				// Anything can be anything.
				self.score_range.end = Bound::Unbounded;
			}
		}

		self.entries.push(entry);
	}

	pub fn import_from(&mut self, other: FeatureList) {
		for entry in other.entries {
			self.push_raw(entry);
		}
	}

	pub fn mandatory_feature<N: Display, D: Display>(
		&mut self,
		desc: FeatureDescriptor<N, D>,
		score: FeatureScore,
	) -> bool {
		let desc = desc.to_strings();
		let is_supported = score.is_met();

		self.push_raw(FeatureEntry {
			desc,
			mandatory: true,
			score,
			sub: None,
		});

		is_supported
	}

	pub fn entries(&self) -> &[FeatureEntry] {
		self.entries.as_slice()
	}

	pub fn met_mandatory(&self) -> usize {
		self.met_mandatory
	}

	pub fn met_optional(&self) -> usize {
		self.met_optional
	}

	pub fn total_mandatory(&self) -> usize {
		self.total_mandatory
	}

	pub fn total_optional(&self) -> usize {
		self.entries.len() - self.total_mandatory
	}

	pub fn missed_mandatory(&self) -> usize {
		self.total_mandatory() - self.met_mandatory()
	}

	pub fn missed_optional(&self) -> usize {
		self.total_optional() - self.met_optional()
	}

	pub fn did_pass(&self) -> bool {
		self.met_mandatory == self.total_mandatory
	}

	pub fn score(&self) -> Option<OrdF32> {
		if self.did_pass() {
			Some(self.score)
		} else {
			None
		}
	}

	pub fn wrap_user_table<T>(self, user_table: T) -> (Self, Option<T>) {
		if self.did_pass() {
			(self, Some(user_table))
		} else {
			(self, None)
		}
	}
}
