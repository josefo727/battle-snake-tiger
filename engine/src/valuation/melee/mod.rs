//! Position assessment for three and four serpents: assessors over a
//! `MeleeBoard`, combined by the same pipeline as the duel terms.

pub mod appetite;
pub mod attrition;
pub mod enclosure;
pub mod finish;
pub mod finisher;
pub mod hunger;
pub mod larder;
pub mod standing;
pub mod territory;
pub mod weights;

use self::appetite::Appetite;
use self::attrition::Attrition;
use self::enclosure::MeleeEnclosure;
use self::finisher::Finisher;
use self::hunger::Hunger;
use self::larder::Larder;
use self::standing::{HeadDanger, Standing};
use self::territory::{MeleeSurvey, Territory};
use self::weights::{DEFAULT_MELEE_PROFILE, MeleeWeights};
use super::weights::{DEFAULT_PROFILE, WeightSheet};
use super::{StandardPipeline, ValuationPipeline, Weighted};
use crate::arena::melee::MeleeBoard;

/// A melee position with its territory survey computed once, so every term
/// that needs the four-source fill reads the same one (growth iteration 8).
#[derive(Clone, Copy, Debug)]
pub struct Surveyed {
    pub board: MeleeBoard,
    pub survey: MeleeSurvey,
}

impl Surveyed {
    #[must_use]
    pub fn new(board: &MeleeBoard) -> Self {
        Self {
            board: *board,
            survey: Territory.survey(board),
        }
    }
}

/// The nine positional terms of the melee valuation, in ledger order.
pub type MeleeTerms = (
    (
        (
            (
                (
                    (
                        (
                            (((), Weighted<Territory>), Weighted<Standing>),
                            Weighted<Hunger>,
                        ),
                        Weighted<Appetite>,
                    ),
                    Weighted<Larder>,
                ),
                Weighted<HeadDanger>,
            ),
            Weighted<Attrition>,
        ),
        Weighted<Finisher>,
    ),
    Weighted<MeleeEnclosure>,
);

pub type MeleePipeline = ValuationPipeline<MeleeTerms>;

/// The leaf valuation of the melee search: the melee terms while three or four
/// seats live, and the duel pipeline (plus the attrition the two departed seats
/// earned) once only one opponent is left, so both scales meet.
#[derive(Clone, Copy, Debug)]
pub struct MeleeValuation {
    melee: MeleePipeline,
    duel: StandardPipeline,
    attrition_seat: i32,
}

impl MeleeValuation {
    /// The valuation the search uses, with both default profiles.
    #[must_use]
    pub fn standard() -> Self {
        Self::with_profiles(&DEFAULT_MELEE_PROFILE, &DEFAULT_PROFILE)
    }

    /// The same terms with the coefficients of `melee` and `duel`, for experiments.
    #[must_use]
    pub fn with_profiles(melee: &MeleeWeights, duel: &WeightSheet) -> Self {
        Self {
            melee: ValuationPipeline::empty()
                .with(Territory, melee.territory_cell)
                .with(Standing, melee.standing_segment)
                .with(Hunger, melee.hunger_urgency)
                .with(Appetite, melee.appetite_step)
                .with(Larder, melee.larder_pellet)
                .with(HeadDanger, melee.head_danger)
                .with(Attrition, melee.attrition_seat)
                .with(Finisher, melee.finisher_step)
                .with(MeleeEnclosure, melee.enclosure_turn),
            duel: StandardPipeline::with_profile(duel),
            attrition_seat: melee.attrition_seat,
        }
    }

    /// The score of a position in which we are alive with at least one opponent.
    #[must_use]
    pub fn score(&self, board: &MeleeBoard) -> i32 {
        if board.alive_count() == 2 {
            self.duel.score(&board.as_duel()) + 2 * self.attrition_seat
        } else {
            self.melee.score(&Surveyed::new(board))
        }
    }

    /// The largest magnitude any score from this valuation could have.
    #[must_use]
    pub fn worst_case_magnitude(&self) -> i32 {
        self.melee
            .worst_case_magnitude()
            .max(self.duel.worst_case_magnitude() + 2 * self.attrition_seat.abs())
    }
}
