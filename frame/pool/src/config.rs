// use crate::Capabilities;
use frame_support::{
	pallet_prelude::*,
	sp_runtime::Perquintill,
};
use scale_info::TypeInfo;
use composable_traits::pool::WeightingMetric;

// Does not derive Copy as asset_ids is a Vector (i.e. the 
//     data resides on the heap) and thus doesn't derive Copy
#[derive(Clone, Encode, Decode, Default, Debug, PartialEq, TypeInfo)]
pub struct PoolInfo<AccountId, AssetId, Balance, CurrencyId> {
	pub manager: AccountId,
	pub asset_ids: Vec<CurrencyId>,

	pub deposit_min:  Perquintill,
	pub deposit_max:  Perquintill,
	pub withdraw_min: Perquintill,
	pub withdraw_max: Perquintill,

	pub weighting_metric: WeightingMetric<AssetId>,
	// pub weights: Weights<CurrencyId>,

	pub lp_token_id: CurrencyId,
	pub lp_circulating_supply: Balance,
}
