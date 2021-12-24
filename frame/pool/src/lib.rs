#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

mod traits;

#[cfg(test)]
mod mocks;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod config;

#[frame_support::pallet]
pub mod pallet {
	// ----------------------------------------------------------------------------------------------------
    //                                       Imports and Dependencies                                      
	// ----------------------------------------------------------------------------------------------------
	use crate::traits::CurrencyFactory;
	
	use codec::{Codec, FullCodec};
	use composable_traits::{
		pool::{
			Deposit, Pool, PoolConfig, Weight, WeightingMetric, 
		},
		vault::{
			Deposit as Duration, Vault, VaultConfig,
		},
	};
	use frame_support::{
		ensure,
		pallet_prelude::*,
		sp_runtime::Perquintill,
		traits::{
			fungibles::{Inspect, Mutate, Transfer},
			tokens::fungibles::MutateHold,
		},
		PalletId,
	};
	use frame_system::{
		ensure_signed, pallet_prelude::OriginFor, Config as SystemConfig,
	};

	use num_integer::Roots;
	use num_traits::SaturatingSub;
	use scale_info::TypeInfo;

	use sp_runtime::{
		helpers_128bit::multiply_by_rational,
		traits::{
			AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedMul, CheckedSub, 
			Convert, One, Zero,
		},
		ArithmeticError,
	};
	use sp_std::fmt::Debug;

	// ----------------------------------------------------------------------------------------------------
    //                                    Declaration Of The Pallet Type                                           
	// ----------------------------------------------------------------------------------------------------

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// ----------------------------------------------------------------------------------------------------
    //                                             Config Trait                                            
	// ----------------------------------------------------------------------------------------------------

	// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		// Loosely couple the Vault trait providing local types for the vaults associated types.
		type Vault: Vault<
			AccountId = Self::AccountId,
			AssetId = Self::AssetId,
			Balance = Self::Balance,
			BlockNumber = Self::BlockNumber
		>;

		// The Balance type used by the pallet for bookkeeping. `Config::Convert` is used for
		// conversions to `u128`, which are used in the computations.
		type Balance: Default
			+ Parameter
			+ Codec
			+ Copy
			+ Ord
			+ CheckedAdd
			+ CheckedSub
			+ CheckedMul
			+ One
			+ Roots
			+ SaturatingSub
			+ AtLeast32BitUnsigned
			+ Zero;

		// The pallet creates new LP tokens for every pool created. It uses `CurrencyFactory`, as
		//     `orml`'s currency traits do not provide an interface to obtain asset ids (to avoid id
		//     collisions).
		type CurrencyFactory: CurrencyFactory<Self::AssetId>;

		// The `AssetId` used by the pallet. Corresponds the the Ids used by the Currency pallet.
		type AssetId: FullCodec
			+ Eq
			+ PartialEq
			+ Copy
			+ MaybeSerializeDeserialize
			+ Debug
			+ Default
			+ TypeInfo;

		// <Self::Vault as Vault>::AssetId
		// Generic Currency bounds. These functions are provided by the `[orml-tokens`](https://github.com/open-web3-stack/open-runtime-module-library/tree/HEAD/currencies) pallet.
		type Currency: Transfer<Self::AccountId, Balance = Self::Balance, AssetId = Self::AssetId>
			+ Mutate<Self::AccountId, Balance = Self::Balance, AssetId = Self::AssetId>
			+ MutateHold<Self::AccountId, Balance = Self::Balance, AssetId = Self::AssetId>;
		
		// Converts the `Balance` type to `u128`, which internally is used in calculations.
		type Convert: Convert<Self::Balance, u128> + Convert<u128, Self::Balance>;

		// The asset ID used to pay for rent.
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		// The native asset fee needed to create a vault.
		#[pallet::constant]
		type CreationFee: Get<Self::Balance>;

		// The deposit needed for a pool to never be cleaned up.
		#[pallet::constant]
		type ExistentialDeposit: Get<Self::Balance>;

		// The margin of error when working with the Pool's weights.
		//     Pool Creation: initial weights, when normalized, must add up into the range
		//         1 - epsilon <= weights <= 1 + epsilon
		//     Pool Deposits: deposit weights, when normalized by the total deposit amount,
		//         must add up into the range 1 - epsilon <= deposit <= 1 + epsilon
		#[pallet::constant]
		type Epsilon: Get<Perquintill>;

		// The minimum allowed amount of assets a user can deposit.
		#[pallet::constant]
		type MinimumDeposit: Get<Self::Balance>;

		// The minimum allowed amount of assets a user can withdraw.
		#[pallet::constant]
		type MinimumWithdraw: Get<Self::Balance>;

		// The id used as the `AccountId` of each pool. This should be unique across all pallets to
		//     avoid name collisions with other pallets.
		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	// ----------------------------------------------------------------------------------------------------
    //                                             Pallet Types                                           
	// ----------------------------------------------------------------------------------------------------

	pub type PoolIndex = u64;

	pub type AssetIdOf<T> =
		<<T as Config>::Currency as Inspect<<T as SystemConfig>::AccountId>>::AssetId;
	
	#[allow(missing_docs)]
	pub type AccountIdOf<T> = <T as SystemConfig>::AccountId;

	#[allow(missing_docs)]
	pub type BlockNumberOf<T> = <T as SystemConfig>::BlockNumber;

	#[allow(missing_docs)]
	pub type BalanceOf<T> = <T as Config>::Balance;

	// Type alias exists mainly since `crate::config::PoolInfo` has many generic parameters.
	pub type PoolInfo<T> =
		crate::config::PoolInfo<AccountIdOf<T>, AssetIdOf<T>, BalanceOf<T>, AssetIdOf<T>>;

	pub type DepositInfo<T> = 
		Deposit<<T as Config>::AssetId, <T as Config>::Balance>;

	// ----------------------------------------------------------------------------------------------------
    //                                           Runtime  Storage                                          
	// ----------------------------------------------------------------------------------------------------

	// The number of active pools - also used to generate the next pool identifier.
	#[pallet::storage]
	#[pallet::getter(fn pool_count)]
	pub type PoolCount<T: Config> = StorageValue<
		_, 
		PoolIndex, 
		ValueQuery
	>;

	// Mapping of a Pool's index to its PoolInfo struct.
	#[pallet::storage]
	#[pallet::getter(fn pool_info_for)]
	pub type Pools<T: Config> = StorageMap<
		_, 
		Twox64Concat, 
		PoolIndex, 
		PoolInfo<T>, 
		ValueQuery
	>;

	#[pallet::storage]
	#[pallet::getter(fn pool_id_and_asset_id_to_weight)]
	pub type PoolIdAndAssetIdToWeight<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIndex,
		Blake2_128Concat,
		T::AssetId,
		Perquintill,
		ValueQuery
	>;

	#[pallet::storage]
	#[pallet::getter(fn pool_id_and_asset_id_to_vault_id)]
	pub type PoolIdAndAssetIdToVaultId<T: Config> = StorageDoubleMap<
		_, 
		Blake2_128Concat, 
		PoolIndex, 
		Blake2_128Concat,
		T::AssetId,
		<T::Vault as Vault>::VaultId,
		ValueQuery
	>;

	#[pallet::storage]
	#[pallet::getter(fn pool_id_to_vault_ids)]
	pub type PoolIdToVaultIds<T: Config> = StorageMap<
		_, 
		Twox64Concat, 
		PoolIndex, 
		Vec<<T::Vault as Vault>::VaultId>,
		ValueQuery
	>;

	#[pallet::storage]
	#[pallet::getter(fn lp_token_to_pool_id)]
	pub type LpTokenToPoolId<T: Config> = StorageMap<
		_, 
		Twox64Concat, 
		T::AssetId, 
		PoolIndex, 
		ValueQuery
	>;

	// ----------------------------------------------------------------------------------------------------
    //                                            Runtime Events                                          
	// ----------------------------------------------------------------------------------------------------

	// Pallets use events to inform users when important changes are made.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// Emitted after a pool has been created. [pool_id]
		PoolCreated {
			// The (incremented) ID of the created pool.
			pool_id: PoolIndex,
		},

		// Emitted after a user deposits assets into a pool.
		Deposited {
			// The account issuing the deposit.
			account: AccountIdOf<T>,
			// The pool deposited into.
			pool_id: PoolIndex,
			// The asset ids and corresponding amount deposited.
			deposited: Vec<DepositInfo<T>>,
			// The number of LP tokens minted for the deposit.
			lp_tokens_minted: BalanceOf<T>,
		},

		// Emitted after a user withdraws assets from a pool.
		Withdrawn {
			// The account issuing the deposit.
			account: AccountIdOf<T>,
			// The pool deposited into.
			pool_id: PoolIndex,
			// The asset ids and corresponding amount withdrawn.
			withdrawn: Vec<DepositInfo<T>>,
			// The number of LP tokens burned from the withdraw.
			lp_tokens_burned: BalanceOf<T>,
		},
	}

	// ----------------------------------------------------------------------------------------------------
    //                                           Runtime  Errors                                           
	// ----------------------------------------------------------------------------------------------------

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		//
		PoolIdsMustBePositive,
		
		// Providing an exiting pool id in the pool_config argument of create results in:
		PoolIdAlreadyExists,

		// Providing a pool id that is greater than the max value for u64 variables results in:
		PoolIdIsTooLarge,
		
		// Creating a pool specifying weights that don't sum to one results in:
		PoolWeightsMustBeNormalized,

		// Creating a vault underneath the pool with invalid creation deposits (i.e. less
		//     than the minimum required native asset to open the vault as Existential plus
	    //     the amount required for the vaults deletion reward) results in: 
		InsufficientDepositToOpenPoolOfThisSize,

		// Trying to create a pool (that represents N assets) when the issuer has less than 
		//     N * (CreationFee + ExistentialDeposit) native tokens results in:
		IssuerDoesNotHaveBalanceTryingToDeposit,

		// Trying to deposit a number of assets not equivalent to the Pool's underlying assets
		//     results in:
		ThereMustBeOneDepositForEachAssetInThePool,

		// Failure to create an LP tokens during pool creation results in:
		ErrorCreatingLpTokenForPool,

		// Users issuing a request with a pool id that doesn't correspond to an active (created) 
		//     Pool results in:
		PoolDoesNotExist,

		// Users depositing assets into a pool with a ratio that doesn't match the ratio from the pools
		//     weighting metric results in:
		DepositDoesNotMatchWeightingMetric,

		// Users trying to deposit an asset amount that is smaller than the Pools configured minimum 
		//     withdraw results in:
		AmountMustBeGreaterThanMinimumDeposit,

		// Users trying to deposit an asset amount that is larger than the Pools configured maximum 
		//     deposit results in:
		AmountMustBeLessThanMaximumDeposit,

		// Users trying to deposit an asset amount that is smaller than the Pools configured maximum 
		//     withdraw results in:
		AmountMustBeLessThanMaximumWithdraw,

		// Users trying to withdraw assets while providing an amount of lp tokens that is smaller than 
		//     T::MinimumWithdraw results in:
		AmountMustBeGreaterThanMinimumWithdraw,

		// Issues that arise when a pool is trying to mint its local lp tokens into the issuers account
		//     results in:
		FailedToMintLpTokens,

		// Issues that arise when transfering funds from the user to pools acocunt results in:
		DepositingIntoPoolFailed,

		//
		IssuerDoesNotHaveLpTokensTryingToDeposit,

		// TODO (Nevin):
		//  - rename to better represent error
		// Issues that arise when transfering tokens from one address to another result in:
		TransferFromFailed,

		// Issues that arise when a pool is trying to burn its local lp tokens from the issuers account
		//     results in:
		FailedToBurnLpTokens,
	}

	// ----------------------------------------------------------------------------------------------------
    //                                              Extrinsics                                             
	// ----------------------------------------------------------------------------------------------------

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	//     These functions materialize as "extrinsics", which are often compared to transactions.
	//     Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// Creates a new Pool, ransfering the deposit into the pool's account. The deposit
		//     must be larger than `NumberOfAssets` x (`ExistentialDeposit` + `CreationFee`)
		//     to guarantee the account will be remained open forever
		// # Emits
		//  - [`Event::PoolCreated`](Event::PoolCreated)
		//
		// # Errors
		//  - When the extrinsic is not signed.
		//  - When any `deposit` < `config.asset_ids.len()` x (`CreationFee` + `ExistentialDeposit`).
		//  - When the issuer has insufficient funds to lock each deposit.
		#[pallet::weight(10_000)]
		pub fn create(
			origin: OriginFor<T>,
			config: PoolConfig<AccountIdOf<T>, AssetIdOf<T>>,
			deposit: Deposit<AssetIdOf<T>, BalanceOf<T>>,
		) -> DispatchResultWithPostInfo {
			// Requirement 0) This extrinsic must be signed 
			let from = ensure_signed(origin)?;

			// Requirement 1) The user who issued the extrinsic has the total balance trying to deposit
			let native_asset_id = T::NativeAssetId::get();
			ensure!(
				T::Currency::balance(native_asset_id, &from) >= deposit.amount, 
				Error::<T>::IssuerDoesNotHaveBalanceTryingToDeposit
			);

			// Requirement 2) For each vault that needs to be opened, there must be a portion of the deposit
			//     large enough to keep the vault alive forever (T::ExistentialDeposit) and 
			//     supply a creation fee (T::CreationFee).

			let required_deposit = Self::required_creation_deposit_for(config.asset_ids.len() as u128)
				.map_err(|_| ArithmeticError::Overflow)?;

			ensure!(
				deposit.amount >= required_deposit,
				Error::<T>::InsufficientDepositToOpenPoolOfThisSize
			);

			let pool_id = <Self as Pool>::create(from, deposit, config)?;
			Self::deposit_event(Event::PoolCreated { 
				pool_id 
				// TODO: add pool info
			});

			Ok(().into())
		}
	
		// Deposit funds in the pool and receive LP tokens in return.
		// # Emits
		//  - Event::Deposited
		//
		// # Errors
		//  - When the origin is not signed.
		//  - When pool_id doesn't correspond to an existing pool.
		//  - When the issuer does not have the specified deposit amounts.
		//  - When `deposit < MinimumDeposit`.
		#[pallet::weight(10_000)]
		pub fn deposit(
			origin: OriginFor<T>,
			pool_id: PoolIndex,
			deposits: Vec<Deposit<T::AssetId, T::Balance>>,
		) -> DispatchResultWithPostInfo {
			// Requirement 0) this extrinsic must be signed 
			let from = ensure_signed(origin)?;

			// Requirement 1) the desired pool index must exist
			ensure!(Pools::<T>::contains_key(pool_id), Error::<T>::PoolDoesNotExist);

			// Requirement 2) the user who issued the extrinsic must have the total balance trying to deposit
			for deposit in &deposits {
				ensure!(
					T::Currency::balance(deposit.asset_id, &from) >= deposit.amount, 
					Error::<T>::IssuerDoesNotHaveBalanceTryingToDeposit
				);
			}

			// Requirement 3) if the deposit amount does not meet the pools weighting metric,
			//     use a DEX to swap an amount of tokens to meet the pools weights.

			// TODO:
			//   - Requirement 3

			let lp_tokens_minted = <Self as Pool>::deposit(&from, &pool_id, deposits.clone())?;
			
			Self::deposit_event(Event::Deposited { 
				account:          from, 
				pool_id:          pool_id, 
				deposited:        deposits, 
				lp_tokens_minted: lp_tokens_minted,
			});

			Ok(().into())
		}

		// Withdraw assets from the pool by supplying the pools native LP token.
		// # Emits
		//  - Event::Withdrawn
		//
		// # Errors
		//  - When the origin is not signed.
		//  - When pool_id doesn't correspond to an existing pool.
		//  - When the issuer does not have the specified lp token amount.
		#[pallet::weight(10_000)]
		pub fn withdraw(
			origin: OriginFor<T>,
			pool_id: PoolIndex,
			lp_amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			// Requirement 0) this extrinsic must be signed 
			let to = ensure_signed(origin)?;

			// Requirement 1) the desired pool index must exist
			ensure!(Pools::<T>::contains_key(pool_id), Error::<T>::PoolDoesNotExist);

			// Requirement 2) the user who issued the extrinsic must have the total balance trying to deposit
			ensure!(
				T::Currency::balance(Self::lp_asset_id(&pool_id)?, &to) >= lp_amount,
				Error::<T>::IssuerDoesNotHaveLpTokensTryingToDeposit
			);
		
			let assets_withdrawn = <Self as Pool>::withdraw(&to, &pool_id, lp_amount)?;
			
			Self::deposit_event(Event::Withdrawn { 
				account:          to, 
				pool_id:          pool_id, 
				withdrawn:        assets_withdrawn, 
				lp_tokens_burned: lp_amount,
			});

			Ok(().into())
		}
	}

	// ----------------------------------------------------------------------------------------------------
    //                                      Pool Trait Implementation                                       
	// ----------------------------------------------------------------------------------------------------

	impl<T: Config> Pool for Pallet<T> {
		type AccountId = T::AccountId;
		type Balance = T::Balance;
		type AssetId = AssetIdOf<T>;

		type PoolId = PoolIndex;

		fn lp_asset_id(pool_id: &Self::PoolId) -> Result<Self::AssetId, DispatchError> {
			Ok(Self::pool_info(pool_id)?.lp_token_id)
		}

		fn account_id(pool_id: &Self::PoolId) -> Self::AccountId {
			T::PalletId::get().into_sub_account(pool_id)
		}

		fn balance_of(pool_id: &Self::PoolId, asset_id: &Self::AssetId) -> Result<Self::Balance, DispatchError> {
			let vault_id = &PoolIdAndAssetIdToVaultId::<T>::get(pool_id, asset_id);
			let vault_lp_token_id = T::Vault::lp_asset_id(&vault_id)?;
		
			let pool_account = &Self::account_id(pool_id);

			Ok(T::Currency::balance(vault_lp_token_id, pool_account))
		}

		fn create(
			from: Self::AccountId,
			deposit: Deposit<Self::AssetId, Self::Balance>,
			config: PoolConfig<Self::AccountId, AssetIdOf<T>>,
		) -> Result<Self::PoolId, DispatchError> {
			Self::do_create_pool(from, deposit, config).map(|(id, _)| id)
		}

		fn deposit(
			from: &Self::AccountId,
			pool_id: &Self::PoolId,
			deposits: Vec<Deposit<Self::AssetId, Self::Balance>>,
		) -> Result<Self::Balance, DispatchError> {
			let pool_info = Self::pool_info(pool_id)?;

			// Requirement 1) The amount of each asset being deposited must be larger than
			//     the pools minimum deposit value and smaller than the pools maximum
			//     deposit value
			
			// TODO (Nevin):
			//  - empty pools should have a minimum deposit not based on the percentage of pools assets

			let lp_circulating_supply = pool_info.lp_circulating_supply;
			if lp_circulating_supply != T::Balance::zero() {
				for deposit in &deposits {
					let pool_balance_of_token = Self::balance_of(pool_id, &deposit.asset_id)?;
	
					// Asset amount to deposit must be greater than the pools deposit minimum
					let min_deposit = Self::percent_of(pool_info.deposit_min, pool_balance_of_token);
					ensure!(
						deposit.amount > min_deposit,
						Error::<T>::AmountMustBeGreaterThanMinimumDeposit
					);
	
					// Asset amount to deposit must be less than the pools deposit maximum
					let max_deposit = Self::percent_of(pool_info.deposit_max, pool_balance_of_token);
					ensure!(
						deposit.amount < max_deposit,
						Error::<T>::AmountMustBeLessThanMaximumDeposit
					);
				}
			}

			// Requirement 2) There must be one deposit for each asset in the pool 
			ensure!(
				pool_info.asset_ids.len() == deposits.len(),
				Error::<T>::ThereMustBeOneDepositForEachAssetInThePool
			);

			for deposit in &deposits {
				ensure!(
					PoolIdAndAssetIdToVaultId::<T>::contains_key(pool_id, &deposit.asset_id),
					Error::<T>::ThereMustBeOneDepositForEachAssetInThePool
				);
			}

			// Requirement 3) Ensure that deposit amount follow the pools weighting metric with some margin of error (T::Epsilon)
			let mut total: u128 = 0;
			deposits.iter().for_each(|deposit| 
				total += <T::Convert as Convert<T::Balance, u128>>::convert(deposit.amount)
			);
			
			// allow for a margin of error
			let epsilon = T::Epsilon::get();

			for deposit in &deposits{				
				let weight = PoolIdAndAssetIdToWeight::<T>::get(&pool_id, deposit.asset_id);
			
				let amount = <T::Convert as Convert<T::Balance, u128>>::convert(deposit.amount);
				ensure!(
					(weight * total) - (epsilon * total) <= amount && amount <= (weight * total) + (epsilon * total),
					Error::<T>::DepositDoesNotMatchWeightingMetric
				);
			}

			Self::do_deposit(from, pool_id, deposits)
		}

		fn withdraw(
			to: &Self::AccountId,
			pool_id: &Self::PoolId,
			lp_amount: Self::Balance,
		) -> Result<Vec<Deposit<Self::AssetId, Self::Balance>>, DispatchError> {
			let pool_info = Self::pool_info(pool_id)?;

			// Requirement 1) The amount of each asset being withdrawn must be larger than
			//     the pools minimum withdraw value and smaller than the pools maximum
			//     withdraw value
			let lp_circulating_supply = pool_info.lp_circulating_supply;

			for asset_id in &pool_info.asset_ids {
				let pool_balance_of_token = Self::balance_of(pool_id, asset_id)?;
				let lp_share_of_asset: T::Balance = Self::calculate_lps_share_of_pools_asset(
					pool_balance_of_token,
					lp_amount,
					lp_circulating_supply
				).map_err(|_| ArithmeticError::Overflow)?;

				// Asset amount to withdraw must be greater than the pools withdraw minimum
				let min_withdraw = Self::percent_of(pool_info.withdraw_min, pool_balance_of_token);
				ensure!(
					lp_share_of_asset > min_withdraw,
					Error::<T>::AmountMustBeGreaterThanMinimumWithdraw
				);

				// Asset amount to withdraw must be less than the pools withdraw maximum
				let max_withdraw = Self::percent_of(pool_info.withdraw_max, pool_balance_of_token);
				ensure!(
					lp_share_of_asset < max_withdraw,
					Error::<T>::AmountMustBeLessThanMaximumWithdraw
				);
			}

			Self::do_withdraw(to, pool_id, lp_amount)
		}

		// Calculates the weights for the pool using the weighting metric specified as a WeightingMetric variant
		fn calculate_weights(
			pool_id: &Self::PoolId,
			asset_ids: &Vec<Self::AssetId>,
			weighting_metric: &WeightingMetric<Self::AssetId>
		) -> Result<(), DispatchError> {
			// Requirement 1) Calculate the pools weights dependant on its specified weighting metric
			let weights: Vec<Weight<Self::AssetId>> = match weighting_metric {
				WeightingMetric::Equal => {
					let mut weights = Vec::<Weight<Self::AssetId>>::new();
					for asset_id in asset_ids {
						weights.push( Weight {
							asset_id: 	*asset_id,
							weight: 	Perquintill::from_float(1 as f64 / asset_ids.len() as f64),
						});
					}
					weights
				},
				WeightingMetric::Fixed(weights) => {
					weights.to_vec()
				},
			};

			// Requirement 2) The weights must be normalized
			// TODO (Nevin):
			//  - (Maybe) enforce that weights sum to one exactly (rather than allowing margin of error)

			let epsilon = T::Epsilon::get().deconstruct();
			let one = Perquintill::one().deconstruct();

			let sum = weights
				.iter()
				.map(|weight| weight.weight.deconstruct())
				.sum();

			ensure!(
				(one - epsilon) <= sum && sum <= (one + epsilon),
				Error::<T>::PoolWeightsMustBeNormalized
			);

			// Requirement 3) Persist PoolId and AssetId to Weight mapping in storage
			for share in &weights {
				PoolIdAndAssetIdToWeight::<T>::insert(&pool_id, share.asset_id, share.weight);
			}

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		// Helper function for the the create extrinsic. Obtains a new LP Token Id for the pool, creates a 
		//     vault for each asset desired in the pool, and saves all important info into storage
		// # Errors
		//  - When their is an issue creating an lp token for the pool.
		//  - When there is an issue creating an underlying vault.
		//  - When any `deposit` < `CreationFee` + `ExistentialDeposit`.
		//  - When the issuer has insufficient funds to lock each deposit.
		fn do_create_pool(
			from: T::AccountId,
			deposit: Deposit<T::AssetId, T::Balance>,
			config: PoolConfig<T::AccountId, AssetIdOf<T>>,
		) -> Result<(PoolIndex, PoolInfo<T>), DispatchError>  {
			PoolCount::<T>::try_mutate(|id| {
				let id = {
					*id += 1;
					*id
				};

				// Requirement 1) Obtain a new asset id for this pools lp token 
				let lp_token_id =
					{ T::CurrencyFactory::create().map_err(|_| Error::<T>::ErrorCreatingLpTokenForPool)? };

				let account_id = Self::account_id(&id);

				// TODO (Nevin):
				//  - allow strategies to be supplied during the creation of the vaults
				//     |-> each vault can have different strategies or there can be one set of
				//     |       strategies for the overall pool
				//     '-> (in PoolConfig) strategies will be a Vec<BTreeMap<AccountId, Perquintill>>
				//             and reserved will be a Vec<Perquintill>

				// Requirement 2) Create a unique vault for each underlying asset
				for asset_id in &config.asset_ids {
					// TODO (Nevin):
					//   - if there is an issue creating the nth vault destroy the previous n-1 vaults

					let vault_id: <T::Vault as Vault>::VaultId = T::Vault::create(
						Duration::Existential,
						VaultConfig::<T::AccountId, T::AssetId> {
							asset_id: *asset_id,
							reserved: Perquintill::from_percent(100),
							manager: account_id.clone(),
							strategies: [].iter().cloned().collect(),
						},
					)?;
					
					PoolIdAndAssetIdToVaultId::<T>::insert(id, asset_id, vault_id.clone());
				}

				// Requirement 3) Transfer native tokens from users account to pools account
				let to = &Self::account_id(&id);
				T::Currency::transfer(deposit.asset_id, &from, to, deposit.amount, true)
					.map_err(|_| Error::<T>::DepositingIntoPoolFailed)?;

				// Requirement 4) Determine each underlying assets corresponding weight
				Self::calculate_weights(&id, &config.asset_ids, &config.weighting_metric)?;

				// Requirement 5) Keep track of the pool's configuration
				let pool_info = PoolInfo::<T> {
					manager:				config.manager,
					asset_ids:				config.asset_ids,

					deposit_min:			config.deposit_min,
					deposit_max:			config.deposit_max,
					withdraw_min:			config.withdraw_min,
					withdraw_max:			config.withdraw_max,
		
					weighting_metric:		config.weighting_metric,

					lp_token_id:			lp_token_id,
					lp_circulating_supply:	T::Balance::zero(),
				};

				Pools::<T>::insert(id, pool_info.clone());
				LpTokenToPoolId::<T>::insert(lp_token_id, id);

				Ok((id, pool_info))
			})
		}

		fn do_deposit(
			from: &T::AccountId,
			pool_id: &PoolIndex,
			deposits: Vec<Deposit<T::AssetId, T::Balance>>,
		) -> Result<T::Balance, DispatchError> {
			
			let mut pool_info = Pools::<T>::get(&pool_id);
			let to = &Self::account_id(pool_id);

			// Requirement 1) Calculate the number of lp tokens to mint as a result of this deposit
			//  |-> when depositig funds to an empty pool, initialize lp token ratio to
			//  |       the gemoetric mean of the amount deposited. (generalizes Uniswap V2's 
			//  |       methodology for minting lp tokens when pool is empty)
			//  '-> when depositing funds to a non-empty pool, the amount of lp tokens minted
			//          should be equivalent to the product of the current curculating supply
			//          of lp tokens for this pool and the ratio of assets deposited to the
			//          pools balance of the tokens

			let lp_tokens_to_mint = if pool_info.lp_circulating_supply == T::Balance::zero() {
				// TODO (Jesper):
				//  - accounting for MIN LIQ too like uniswap (to avoid sybil, and other issues)

				Self::geometric_mean(&deposits)
			} else {
				Self::calculate_lp_tokens_to_mint(&pool_id, &deposits)
			}.map_err(|_| ArithmeticError::Overflow)?;

			// TODO (Nevin):
			//  - transfer all assets into the pool's account before beginning
			//     '-> should be its own function to abstract away details

			// Requirement 2) Deposit each asset into the pool's underlying vaults
			for deposit in &deposits {
				// TODO (Nevin):
				//  - check for errors in vault depositing, and if so revert all deposits so far
				//     '-> surround T::Vault::deposit call in match statement checking for Ok(lp_token_amount)
				//             and DispatchError

				let vault_id = &PoolIdAndAssetIdToVaultId::<T>::get(pool_id, deposit.asset_id);

				T::Currency::transfer(deposit.asset_id, &from, to, deposit.amount, true)
					.map_err(|_| Error::<T>::DepositingIntoPoolFailed)?;

				let _vault_lp_token_amount = T::Vault::deposit(
					vault_id,
					to,
					deposit.amount,
				)?;
			}

			// Requirement 3) Mint Pool's lp tokens into the issuers account and update 
			//     circulating supply of lp tokens

			T::Currency::mint_into(pool_info.lp_token_id, from, lp_tokens_to_mint)
				.map_err(|_| Error::<T>::FailedToMintLpTokens)?;

			pool_info.lp_circulating_supply = pool_info.lp_circulating_supply + lp_tokens_to_mint;
			Pools::<T>::insert(&pool_id, pool_info);

			Ok(lp_tokens_to_mint)
		}

		fn do_withdraw(
			to: &T::AccountId,
			pool_id: &PoolIndex,
			lp_amount: T::Balance,
		) -> Result<Vec<Deposit<T::AssetId, T::Balance>>, DispatchError> {
			let pool_account = &Self::account_id(pool_id);

			let mut pool_info = Pools::<T>::get(&pool_id);
			let lp_circulating_supply = pool_info.lp_circulating_supply;

			// Used to keep track of the amount of each asset withdrawn from the pool's underlying vaults
			let mut assets_withdrawn = Vec::<Deposit<T::AssetId, T::Balance>>::new();

			// Requirement 1) Calculate and withdraw the lp tokens share of the each asset in the pool
			for asset_id in &pool_info.asset_ids {
				let vault_id = &PoolIdAndAssetIdToVaultId::<T>::get(pool_id, asset_id);
				let pool_balance_of_token = Self::balance_of(pool_id, asset_id)?;

				// Calculate the percentage of the pool's assets that correspond to the deposited lp tokens
				let lp_share_of_asset: T::Balance = Self::calculate_lps_share_of_pools_asset(
					pool_balance_of_token,
					lp_amount,
					lp_circulating_supply
				).map_err(|_| ArithmeticError::Overflow)?;

				// Withdraw the vaults assets into the pools account
				let vault_balance_withdrawn = T::Vault::withdraw(
					vault_id,
					&pool_account,
					lp_share_of_asset
				)?;
				// Withdraw the assets now in the pools account into the issuers account
				T::Currency::transfer(*asset_id, pool_account, to, vault_balance_withdrawn, true)
					.map_err(|_| Error::<T>::TransferFromFailed)?;

				assets_withdrawn.push(
					Deposit {
						asset_id: *asset_id,
						amount: vault_balance_withdrawn,
					}
				);
			}

			// Requirement 2) burn the lp tokens that were deposited during this withdraw
			T::Currency::burn_from(pool_info.lp_token_id, to, lp_amount)
				.map_err(|_| Error::<T>::FailedToBurnLpTokens)?;
			
			// Update the pools counter of the circulating supply of lp tokens to subtract the amount burned
			pool_info.lp_circulating_supply = pool_info.lp_circulating_supply - lp_amount;
			Pools::<T>::insert(&pool_id, pool_info);

			Ok(assets_withdrawn)
		}
	}

	// Helper functions for core functionality
	impl<T: Config> Pallet<T> {
		fn pool_info(pool_id: &PoolIndex) -> Result<PoolInfo<T>, DispatchError> {
			Ok(Pools::<T>::try_get(pool_id).map_err(|_err| Error::<T>::PoolDoesNotExist)?)
		}

		// Calculates the geometric mean of the deposit amounts provided
		//  '-> geometric mean = nth-√(Π ai), where 1 ≤ i ≤ n, and ai is the ith asset balance
		//
		// # Errors
		//  - When calculating the product of the asset amounts results in an overflow error.
		fn required_creation_deposit_for(number_of_assets: u128) -> Result<T::Balance, DispatchError> {
			let existential_deposit = T::ExistentialDeposit::get();
			let creation_fee = T::CreationFee::get();

			let number_of_assets = 
				<T::Convert as Convert<u128, T::Balance>>::convert(number_of_assets);
				
			let require_deposit = creation_fee
				.checked_add(&existential_deposit).ok_or(ArithmeticError::Overflow)?
				.checked_mul(&number_of_assets).ok_or(ArithmeticError::Overflow)?;
					
			Ok(require_deposit)
		}

		// Calculates the geometric mean of the deposit amounts provided
		//  '-> geometric mean = nth-√(Π ai), where 1 ≤ i ≤ n, and ai is the ith asset balance
		//
		// # Errors
		//  - When calculating the product of the asset amounts results in an overflow error.
		fn geometric_mean(deposits: &Vec<Deposit<T::AssetId, T::Balance>>) -> Result<T::Balance, DispatchError> {
			let number_of_assets = deposits.len() as u32;
			let mut result = T::Balance::one();
		
			for deposit in deposits {
				result = result.checked_mul(&deposit.amount).ok_or(ArithmeticError::Overflow)?;
			}
					
			Ok(result.nth_root(number_of_assets))
		}

		// Calculates the number of lp tokens to mint from the deposit amounts provided
		//  '-> lp tokens to mint = lp_circulating_supply * (ai/balance_i), where 1 ≤ i ≤ n
		//
		// # Errors
		//  - When calculating the product of the asset ratio and the supply of lp tokens amounts results in an overflow error.
		fn calculate_lp_tokens_to_mint(
			pool_id: &PoolIndex,
			deposits: &Vec<Deposit<T::AssetId, T::Balance>>
		) -> Result<T::Balance, DispatchError> {
			// let lp_circulating_supply = Pools::<T>::get(pool_id).lp_circulating_supply;

			// let mut lp_tokens_to_mint = T::Balance::zero();

			// for deposit in deposits {
			// 	let weight = PoolIdAndAssetIdToWeight::<T>::get(pool_id, deposit.asset_id);
				
			// 	let vault_id = &PoolIdAndAssetIdToVaultId::<T>::get(pool_id, deposit.asset_id);
			// 	let vault_account = &<T::Vault as Vault>::account_id(vault_id);

			// 	let pool_balance_of_token = T::Currency::balance(deposit.asset_id, vault_account);

			// 	let lp_tokens_to_mint_for_asset = multiply_by_rational(
			// 		<T::Convert as Convert<T::Balance, u128>>::convert(lp_circulating_supply),
			// 		<T::Convert as Convert<T::Balance, u128>>::convert(deposit.amount),
			// 		<T::Convert as Convert<T::Balance, u128>>::convert(pool_balance_of_token),
			// 	).map_err(|_| ArithmeticError::Overflow)?;

			// 	let lp_tokens_to_mint_for_asset = weight * lp_tokens_to_mint_for_asset;

			// 	let lp_tokens_to_mint_for_asset =
			// 		<T::Convert as Convert<u128, T::Balance>>::convert(lp_tokens_to_mint_for_asset);

			// 	lp_tokens_to_mint = lp_tokens_to_mint.checked_add(&lp_tokens_to_mint_for_asset)
			// 		.ok_or(ArithmeticError::Overflow)?;

			// }

			// Ok(lp_tokens_to_mint)

			// TODO (Nevin):
			//  - generalize this formula to sum all deposit rations multiplied by their weights
			//  - check that this formula is still accurate when weights are off balance before rebalancing
			//    '-> lp_minted = lp_circulating_supply * 
			//            sum of (token i's weight * (deposit of token i / balance of token i in pool))
			let lp_circulating_supply = Pools::<T>::get(pool_id).lp_circulating_supply;
			
			let deposit = &deposits[0];
			let vault_id = &PoolIdAndAssetIdToVaultId::<T>::get(pool_id, deposit.asset_id);
			let vault_id = &<T::Vault as Vault>::account_id(vault_id);

			let pool_balance_of_token = T::Currency::balance(deposit.asset_id, vault_id);
			let deposit_ratio = multiply_by_rational(
				<T::Convert as Convert<T::Balance, u128>>::convert(lp_circulating_supply),
				<T::Convert as Convert<T::Balance, u128>>::convert(deposit.amount),
				<T::Convert as Convert<T::Balance, u128>>::convert(pool_balance_of_token),
			).map_err(|_| ArithmeticError::Overflow)?;

			Ok(<T::Convert as Convert<u128, T::Balance>>::convert(deposit_ratio))
		}

		// Calculates the exact balance of assets that the provided lp tokens (shares) correspond to
		//  '-> LP Share = pool_balance_of_token * (1 - (lp_circulating_supply-lp_amount)/lp_circulating_supply)
		//
		// # Errors
		//  - When calculating the LP Share amount results in an overflow error.
		fn calculate_lps_share_of_pools_asset(
			pool_balance_of_token: T::Balance,
			lp_amount: T::Balance,
			lp_circulating_supply: T::Balance,
		) -> Result<T::Balance, DispatchError> {
			// Convert all three arguments to u128
			let pool_balance_of_token: u128 = 
				<T::Convert as Convert<T::Balance, u128>>::convert(pool_balance_of_token);

			let lp_amount: u128 = 
				<T::Convert as Convert<T::Balance, u128>>::convert(lp_amount);

			let lp_circulating_supply: u128 = 
				<T::Convert as Convert<T::Balance, u128>>::convert(lp_circulating_supply);

			let lp_circulating_minus_amount = lp_circulating_supply
				.checked_sub(lp_amount).ok_or(ArithmeticError::Overflow)?;

			// Calculate the LP Share amount
			let lp_share_of_asset = multiply_by_rational(
				pool_balance_of_token,
				lp_circulating_minus_amount,
				lp_circulating_supply
			).map_err(|_| ArithmeticError::Overflow)?;

			let lp_share_of_asset = pool_balance_of_token
				.checked_sub(lp_share_of_asset).ok_or(ArithmeticError::Overflow)?;

			// Convert back to Balance type
			Ok(<T::Convert as Convert<u128, T::Balance>>::convert(lp_share_of_asset))
		} 
	
		// Calculates the percentage of the given balance and returns it as Balance type
		fn percent_of(
			percentage: Perquintill,
			balance: T::Balance
		) -> T::Balance {
			let balance: u128 = 
				<T::Convert as Convert<T::Balance, u128>>::convert(balance);
	
			<T::Convert as Convert<u128, T::Balance>>::convert(percentage * balance)
		}
	}

}