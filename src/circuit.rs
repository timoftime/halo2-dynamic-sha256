use getset::CopyGetters;
use halo2_base::{
    gates::{
        circuit::{
            builder::BaseCircuitBuilder, BaseCircuitParams, BaseConfig, CircuitBuilderStage,
        },
        flex_gate::threads::SinglePhaseCoreManager,
        RangeChip,
    },
    halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner},
        plonk::{Circuit, ConstraintSystem, Error},
    },
    utils::BigPrimeField,
    virtual_region::manager::VirtualRegionManager,
    Context,
};

use crate::{
    gate::{ShaThreadBuilder, FIRST_PHASE, ShaContexts},
    spread::SpreadConfig,
};

const MAX_PHASE: usize = 3;

#[derive(Debug, Clone)]
pub struct SHAConfig<F: BigPrimeField> {
    pub compression: SpreadConfig<F>,
    pub base: BaseConfig<F>,
}

impl<F: BigPrimeField> SHAConfig<F> {
    pub fn configure(meta: &mut ConstraintSystem<F>, params: BaseCircuitParams) -> Self {
        let degree = params.k;
        let mut base = BaseConfig::configure(meta, params);
        let compression = SpreadConfig::configure(meta, 8, 1);

        // base.gate.max_rows = (1 << degree) - meta.minimum_rows();
        Self { base, compression }
    }
}

#[derive(Clone, Debug, Default, CopyGetters)]
pub struct ShaCircuitBuilder<F: BigPrimeField> {
    #[getset(get = "pub", get_mut = "pub")]
    pub(crate) sha: ShaThreadBuilder<F>,
    #[getset(get = "pub", get_mut = "pub")]
    pub(crate) base: BaseCircuitBuilder<F>,
}

impl<F: BigPrimeField> ShaCircuitBuilder<F> {
    pub fn new(witness_gen_only: bool) -> Self {
        let base = BaseCircuitBuilder::new(witness_gen_only);
        Self {
            sha: ShaThreadBuilder::new(witness_gen_only)
                .use_copy_manager(base.core().phase_manager[FIRST_PHASE].copy_manager.clone()),
            base,
        }
    }

    pub fn from_stage(stage: CircuitBuilderStage) -> Self {
        Self::new(stage == CircuitBuilderStage::Prover)
            .unknown(stage == CircuitBuilderStage::Keygen)
    }

    pub fn unknown(mut self, use_unknown: bool) -> Self {
        self.sha = self.sha.unknown(use_unknown);
        self.base = self.base.unknown(use_unknown);
        self
    }

    /// Creates a new [ShaCircuitBuilder] with `use_unknown` of [ShaThreadBuilder] set to true.
    pub fn keygen() -> Self {
        Self::from_stage(CircuitBuilderStage::Keygen)
    }

    /// Creates a new [ShaCircuitBuilder] with `use_unknown` of [GateThreadBuilder] set to false.
    pub fn mock() -> Self {
        Self::from_stage(CircuitBuilderStage::Mock)
    }

    /// Creates a new [ShaCircuitBuilder].
    pub fn prover() -> Self {
        Self::from_stage(CircuitBuilderStage::Prover)
    }

    /// The log_2 size of the lookup table, if using.
    pub fn lookup_bits(&self) -> Option<usize> {
        self.base.lookup_bits()
    }

    /// Set lookup bits
    pub fn set_lookup_bits(&mut self, lookup_bits: usize) {
        self.base.set_lookup_bits(lookup_bits);
    }

    /// Returns new with lookup bits
    pub fn use_lookup_bits(mut self, lookup_bits: usize) -> Self {
        self.set_lookup_bits(lookup_bits);
        self
    }

    /// Sets new `k` = log2 of domain
    pub fn set_k(&mut self, k: usize) {
        self.base.set_k(k);
    }

    /// Returns new with `k` set
    pub fn use_k(mut self, k: usize) -> Self {
        self.set_k(k);
        self
    }

    /// Set config params
    pub fn set_params(&mut self, params: BaseCircuitParams) {
        self.base.set_params(params)
    }

    /// Returns new with config params
    pub fn use_params(mut self, params: BaseCircuitParams) -> Self {
        self.set_params(params);
        self
    }

    /// Returns a mutable reference to the [Context] of a gate thread. Spawns a new thread for the given phase, if none exists.
    /// * `phase`: The challenge phase (as an index) of the gate thread.
    pub fn main(&mut self) -> &mut Context<F> {
        self.base.main(0)
    }

    /// Returns [SinglePhaseCoreManager] with the virtual region with all core threads in the given phase.
    pub fn pool(&mut self, phase: usize) -> &mut SinglePhaseCoreManager<F> {
        self.base.pool(0)
    }

    pub fn calculate_params(&mut self, minimum_rows: Option<usize>) -> BaseCircuitParams {
        self.base.calculate_params(minimum_rows)
    }

    pub fn sha_contexts_pair(&mut self) -> (&mut Context<F>, ShaContexts<F>) {
        (self.base.main(0), self.sha.contexts_pair())
    }

    pub fn range_chip(&mut self, lookup_bits: usize) -> RangeChip<F> {
        self.base.set_lookup_bits(lookup_bits);
        self.base.range_chip()
    }
}

impl<F: BigPrimeField> Circuit<F> for ShaCircuitBuilder<F> {
    type Config = SHAConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.base.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        unimplemented!()
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        SHAConfig::configure(meta, params)
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> SHAConfig<F> {
        unreachable!("You must use configure_with_params");
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // if let MaybeRangeConfig::WithRange(config) = &config.base.base {
        //     config
        //         .load_lookup_table(&mut layouter)
        //         .expect("load lookup table should not fail");
        // }

        // let mut first_pass = SKIP_FIRST_PASS;
        // let witness_gen_only = self.builder.borrow().witness_gen_only();

        // let mut assigned_advices = HashMap::new();

        config.compression.load(&mut layouter)?;

        layouter.assign_region(
            || "ShaCircuitBuilder generated circuit",
            |mut region| {
                // if first_pass {
                //     first_pass = false;
                //     return Ok(());
                // }

                self.sha.assign_raw(&config.compression, &mut region);
                Ok(())
            },
        )?;

        self.base.synthesize(config.base.clone(), layouter)?;

        Ok(())
    }
}