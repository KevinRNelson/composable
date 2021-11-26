//Bribe DAO

pub use pallet::*;

pub mod sortedvec;
pub mod tests;
pub use crate::sortedvec::FastMap;

#[frame_support::pallet]
pub mod pallet {
	use crate::sortedvec::{BribesStorage, FastMap};
	use codec::Codec;
	use composable_traits::{bribe::Bribe, democracy::Democracy};
	use frame_support::{
		pallet_prelude::*,
		traits::fungibles::{InspectHold, MutateHold, Transfer},
		transactional,
	};
	use frame_system::pallet_prelude::*;
	use num_traits::{CheckedAdd, CheckedMul, CheckedSub, SaturatingSub};
	use pallet_democracy::Vote;
	use sp_runtime::traits::{AtLeast32BitUnsigned, Zero};
	use sp_std::fmt::Debug;
	use std::convert::TryInto;

	pub type BribeIndex = u32;
	pub type ReferendumIndex = pallet_democracy::ReferendumIndex;

	// User asks to buy X amount of votes for a certain amount | Briber
	pub type CreateBribeRequest<T> = composable_traits::bribe::CreateBribeRequest<
		<T as frame_system::Config>::AccountId,
		ReferendumIndex,
		<T as Config>::Balance,
		<T as Config>::Conviction,
		<T as Config>::CurrencyId,
	>;

	// Bribe'e, the user selling its vote for tokens
	pub type TakeBribeRequest<T> = composable_traits::bribe::TakeBribeRequest<
		BribeIndex,
		<T as Config>::Balance,
		<T as Config>::Conviction,
	>;

	pub type DeleteBribeRequest = composable_traits::bribe::DeleteBribeRequest<BribeIndex>;

	// Status of Bribe request
	#[derive(Copy, Clone, Encode, Decode, PartialEq, RuntimeDebug)]
	pub enum BribeStatuses {
		Created,
		Started,
		OnHold,
		Failed,
		Finished,
		InvalidId,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Balance: Default
			+ Parameter
			+ Codec
			+ Copy
			+ Ord
			+ CheckedAdd
			+ CheckedSub
			+ CheckedMul
			+ SaturatingSub
			+ AtLeast32BitUnsigned
			+ Zero;

		type VaultId: Clone + Codec + Debug + PartialEq + Default + Parameter;

		// Currency config supporting transfer, freezing and inspect
		type Currency: Transfer<Self::AccountId, Balance = Self::Balance, AssetId = Self::CurrencyId>
			+ MutateHold<Self::AccountId, Balance = Self::Balance, AssetId = Self::CurrencyId>
			+ InspectHold<Self::AccountId, Balance = Self::Balance, AssetId = Self::CurrencyId>;

		type Conviction: Parameter;

		type Democracy: Democracy<
			AccountId = Self::AccountId,
			ReferendumIndex = pallet_democracy::ReferendumIndex,
			Vote = pallet_democracy::Vote,
		>;

		// TODO(oleksii): CurrencyId traits
		type CurrencyId: Parameter;

		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		// TODO(oleksii): WeightInfo type
		// type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BribeCreated { id: BribeIndex, request: CreateBribeRequest<T> },
		BribeTaken { id: BribeIndex, request: TakeBribeRequest<T> },
		DeleteBribe { id: BribeIndex, request: DeleteBribeRequest },
	}

	/// The number of bribes, also used to generate the next bribe identifier.
	///
	/// # Note
	///
	/// Cleaned up bribes do not decrement the counter.
	#[pallet::storage]
	#[pallet::getter(fn bribe_count)]
	pub(super) type BribeCount<T: Config> = StorageValue<_, BribeIndex, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn fast_vexc)]
	pub(super) type Fastvec<T: Config> = StorageValue<_, FastMap, ValueQuery>; // using value query instead of OptionQuery cuz OptionsQuery returns null if its empty

	#[pallet::storage]
	#[pallet::getter(fn bribe_requests)]
	pub(super) type BribeRequests<T: Config> =
		StorageMap<_, Blake2_128Concat, BribeIndex, CreateBribeRequest<T>>;

	/*
		// Create a cubic vault for holding funds
		pub fn create_vault<T: Config>(
			origin: OriginFor<T>,
			asset_id: T::CurrencyId,
		) -> (T::VaultId, VaultInfo<T::AccountId, T::Balance, T::CurrencyId, T::BlockNumber>) where <T as frame_system::Config>::Origin: Ord {
			Vault::<
				AccountId = T::AccountId,
				AssetId = T::CurrencyId,
				BlockNumber = T::BlockNumber,
				VaultId = T::VaultId,
				Balance = T::Balance,
			>::do_create_vault(
				Deposit::Existential,
				VaultConfig {
					asset_id,
					manager: origin,
					reserved: Perquintill::from_percent(100),
					strategies: [].iter().cloned().collect(),
				},
			);
		}

	*/

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create_bribe(
			origin: OriginFor<T>,
			request: CreateBribeRequest<T>,
		) -> DispatchResultWithPostInfo {
			let _from = ensure_signed(origin)?;
			let id = <Self as Bribe>::create_bribe(request.clone())?;
			Self::deposit_event(Event::BribeCreated { id, request });
			Ok(().into())
		}

		//		#[transactional]
		//		#[pallet::weight(10_000)]
		//		pub fn deposit_funds(
		//			origin: OriginFor<T>,
		//			bribe: BribeIndex,
		//			amount: u128,
		//		) -> DispatchResult {
		//			transfer(account_id, origin, amount);
		//			todo!("deposit_tokens into vault ");

		//			todo!("transfer funds");
		//			todo!("Update token funds status");

		//			Ok(())
		//		insert}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn release_funds(origin: OriginFor<T>, bribe: BribeIndex) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let og_request =
				BribeRequests::<T>::try_get(bribe).map_err(|_| Error::<T>::InvalidBribe)?;
			let amount = og_request.total_reward; // amount of tokens locked in
			let currencyid = og_request.asset_id;
			T::Currency::release(currencyid, &who, amount, false)
				.map_err(|_| Error::<T>::ReleaseFailed)?;
			// remove from fastvec

			//			todo!("Check token supply, if supply is less or same as asked for: release funds");
			//			Error::<T>::EmptySupply;
			//			todo!("update capital status");
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn take_bribe(
			origin: OriginFor<T>,
			request: TakeBribeRequest<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let bribe_index = request.bribe_index;
			let bribe_taken = <Self as Bribe>::take_bribe(request.clone())?;
			let og_request = BribeRequests::<T>::get(request.bribe_index).unwrap(); // should be saved in the create bribe request, if its not then there is a logic error
																		// somewhere, so unwrap should be okey to use
			let amount = og_request.total_reward; // amount of tokens locked in
			let currencyid = og_request.asset_id;
			T::Currency::hold(currencyid, &who, amount).map_err(|_| Error::<T>::CantFreezeFunds)?; //Freeze assets

			if bribe_taken {
				Self::deposit_event(Event::BribeTaken { id: bribe_index, request });
			}
			Ok(().into())
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		InvalidBribe,
		InvalidIndex,
		NotEnoughFunds,
		NotEnoughStake,
		PriceNotRequested,
		AlreadyBribed,
		EmptySupply,
		CantFreezeFunds,
		ReleaseFailed,
	}

	// offchain indexing
	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn offchain_worker(_b: T::BlockNumber) {
			log::info!("Indexing request offchain");
		}
	}

	impl<T: Config> Bribe for Pallet<T> {
		type BribeIndex = BribeIndex;
		type AccountId = T::AccountId;
		type ReferendumIndex = ReferendumIndex;
		type Balance = T::Balance;
		type Conviction = T::Conviction;
		type CurrencyId = T::CurrencyId;

		//		fn lockup_funds(origin: Origin<T>, request: CreateBribeRequest<T>) -> Result<bool,
		// DispatchError>{ 			todo!("lock up users funds until vote is finished");
		//		}

		//		fn payout_funds()

		/// Register new bribe request
		fn create_bribe(request: CreateBribeRequest<T>) -> Result<Self::BribeIndex, DispatchError> {
			Self::do_create_bribe(request)
		}

		/// Register the votes a user wants to sell
		fn take_bribe(request: TakeBribeRequest<T>) -> Result<bool, DispatchError> {
			Self::do_take_bribe(request)
		}

		/// Delete a finished Bribe Request
		fn delete_bribe(request: DeleteBribeRequest) -> Result<bool, DispatchError> {
			// todo, make sure this function can not be abused
			Self::do_delete_bribe(request)
			//			Ok(true)
		}
	}

	impl<T: Config> Pallet<T> {
		/// Create a new Bribe Request
		fn do_create_bribe(request: CreateBribeRequest<T>) -> Result<BribeIndex, DispatchError> {
			let id = BribeCount::<T>::mutate(|id| {
				*id += 1;
				*id
			});

			ensure!(!BribeRequests::<T>::contains_key(id), Error::<T>::AlreadyBribed); //dont duplicate briberequest if we already have it

			BribeRequests::<T>::insert(id, request);
			Ok(id)
		}

		/// Find votes for a bribe request
		fn do_match_votes(bribe_index: BribeIndex) -> Result<bool, DispatchError> {
			ensure!(BribeRequests::<T>::contains_key(bribe_index), Error::<T>::InvalidIndex);

			let bribe_request = BribeRequests::<T>::get(bribe_index).unwrap();

			let ref_index = bribe_request.ref_index;
			// Yield all the bribe votes for sale with the same ref index

			let loot: Vec<BribesStorage> = Fastvec::<T>::get().find_all_pid(ref_index);

			if !loot.is_empty() {
				for bribes in loot {
// Cast Vote, Remove from storage

// Pay out to voters


						let p_id = bribes.p_id;
						let amount = bribes.amount; 
						let votes = bribes.votes;
					}
			}
			Ok(true)
		}

		/// Take Bribe user sell votes request   
		fn do_take_bribe(request: TakeBribeRequest<T>) -> Result<bool, DispatchError> {
			ensure!(
				BribeRequests::<T>::contains_key(request.bribe_index),
				Error::<T>::InvalidIndex
			);
			// todo: make sure the user is not selling the same vote twice

			let bribe_request = BribeRequests::<T>::get(request.bribe_index).unwrap();

			let pid = bribe_request.ref_index; // save based on the referendumIndex
			let amount_votes: u32 = 3;
			let amount: u32 = TryInto::<u32>::try_into(bribe_request.total_reward).ok().unwrap(); // amount of tokens locked in
																					  // insert into fastvec
			Fastvec::<T>::mutate(|a| a.add(amount, pid, amount_votes));

			//			let vote = Vote { aye: bribe_request.is_aye, conviction: Default::default() }; //todo
			// get conviction 			T::Democracy::vote(bribe_request.account_id, bribe_request.ref_index,
			// vote); //AccountId, Referendum Index, Vote
			Ok(true)
			//			todo!("enact vote through pallet_democracy");
		}

		/// Delete Bribe Request
		/// Check the bribe id, delete from BribeRequests and from FastMap
		fn do_delete_bribe(request: DeleteBribeRequest) -> Result<bool, DispatchError> {
			let bribe_id = request.bribe_index;
			// Check if the bribe request id exists
			ensure!(!BribeRequests::<T>::contains_key(bribe_id), Error::<T>::InvalidBribe);

			// Remove from BribeRequests Storage Map
			BribeRequests::<T>::remove(bribe_id);
			// Emit the event
			Self::deposit_event(Event::DeleteBribe { id: bribe_id, request });
			Ok(true)
		}
	}
}
