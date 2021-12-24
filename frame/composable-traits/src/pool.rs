use codec::Codec;
use frame_support::{
	pallet_prelude::*,
	sp_std::fmt::Debug,
	sp_runtime::Perquintill,
};
use scale_info::TypeInfo;

// Holds the id of an asset and how much weight is given to it
//     proportional to all underlying Pool assets.
#[derive(Clone, Encode, Decode, Default, Debug, PartialEq, TypeInfo)]
pub struct Weight<CurrencyId> {
	pub asset_id: CurrencyId,
	pub weight: Perquintill,
}

// // Holds the mapping of an id of an asset and how much weight is given to it
// //     proportional to all underlying Pool assets.
// pub type Weights<CurrencyId> = HashMap::<CurrencyId, Perquintill>;

#[derive(Clone, Encode, Decode, Debug, PartialEq, TypeInfo)]
pub enum WeightingMetric<CurrencyId> {
    Equal,
	Fixed(Vec<Weight<CurrencyId>>)
}
impl<CurrencyId> Default for WeightingMetric<CurrencyId> {
	fn default() -> Self {
		WeightingMetric::Equal
	}
}

// Holds the id of an asset and how much of it is being deposited
//     Pool functions accept a Vec<Deposit> as an argument.
#[derive(Clone, Encode, Decode, Default, Debug, PartialEq, TypeInfo)]
pub struct Deposit<CurrencyId, Balance> {
	pub asset_id: CurrencyId,
	pub amount: Balance,
}

#[derive(Clone, Encode, Decode, Default, Debug, PartialEq, TypeInfo)]
pub struct PoolConfig<AccountId, CurrencyId>
where
	AccountId: core::cmp::Ord,
{
	pub manager: AccountId,
	
	pub asset_ids: Vec<CurrencyId>,
	pub deposit_min:  Perquintill,
	pub deposit_max:  Perquintill,
	pub withdraw_min: Perquintill,
	pub withdraw_max: Perquintill,

    pub weighting_metric: WeightingMetric<CurrencyId>,
    // pub reserved: Perquintill,
	// pub strategies: BTreeMap<AccountId, Perquintill>,
}

pub trait Pool {
	type AccountId: core::cmp::Ord;
	type AssetId;
	type Balance;
	
	type PoolId: Clone + Codec + Debug + PartialEq + Default + Parameter;

	fn lp_asset_id(pool_id: &Self::PoolId) -> Result<Self::AssetId, DispatchError>;

	fn account_id(pool_id: &Self::PoolId) -> Self::AccountId;

	fn balance_of(pool_id: &Self::PoolId, asset_id: &Self::AssetId) -> Result<Self::Balance, DispatchError>;

	// Used by users to create a new pool with the sepcified configuration.
	fn create(
		from: Self::AccountId,
		deposit: Deposit<Self::AssetId, Self::Balance>,
		config: PoolConfig</*Self::PoolId, */Self::AccountId, Self::AssetId>,
	) -> Result<Self::PoolId, DispatchError>;

	// Used by users to deposit tokens into the pool. Returns the true amount of 
	//     lp token minted to user.
	fn deposit(
		from: &Self::AccountId,
		pool_id: &Self::PoolId,
		deposits: Vec<Deposit<Self::AssetId, Self::Balance>>,
	) -> Result<Self::Balance, DispatchError>;

	// Used by users to deposit lp tokens into the pool and withdraw the equivalent 
	//     share of the pools assets. Returns a Vector containing the asset id and 
	//     balance of assets withdrawn.
	fn withdraw(
		to: &Self::AccountId,
		pool_id: &Self::PoolId,
		lp_amount: Self::Balance,
	) -> Result<Vec<Deposit<Self::AssetId, Self::Balance>>, DispatchError>;

	// Used by users to calculate the weights for the pools underlying assets given a 
	//     specific weighting metric
	fn calculate_weights(
		pool_id: &Self::PoolId,
		asset_ids: &Vec<Self::AssetId>,
		weighting_metric: &WeightingMetric<Self::AssetId>
	) -> Result<(), DispatchError>;
}
