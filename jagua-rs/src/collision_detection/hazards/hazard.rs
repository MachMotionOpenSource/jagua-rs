use crate::entities::{PItemKey, PlacedItem};
use crate::geometry::DTransformation;
use crate::geometry::geo_enums::{GeoPosition, GeoRelation};
use crate::geometry::geo_traits::CollidesWith;
use crate::geometry::primitives::SPolygon;
use slotmap::new_key_type;
use std::borrow::Borrow;
use std::sync::Arc;

new_key_type! {
    /// Key to identify hazards inside the CDE.
    pub struct HazKey;
}

/// Any spatial constraint affecting the feasibility of a placement of an Item.
/// See [`HazardEntity`] for the different entities that can induce a hazard.
#[derive(Clone, Debug)]
pub struct Hazard {
    /// The entity inducing the hazard
    pub entity: HazardEntity,
    /// The shape of the hazard
    pub shape: Arc<SPolygon>,
    /// Whether the hazard is dynamic, meaning it can change over time (e.g., moving items)
    pub dynamic: bool,
    /// Optional "safe" inner regions of the hazard, in the same coordinate space as `shape`.
    /// Candidates that are fully contained inside one of these regions are considered
    /// non-colliding with this hazard. Used to support items with holes: a small item
    /// placed inside the hole of a larger item is feasible.
    pub holes: Vec<Arc<SPolygon>>,
}

impl Hazard {
    #[must_use]
    pub fn new(entity: HazardEntity, shape: Arc<SPolygon>, dynamic: bool) -> Self {
        Self {
            entity,
            shape,
            dynamic,
            holes: vec![],
        }
    }

    /// Builder that attaches inner "safe" regions (holes) to this hazard.
    #[must_use]
    pub fn with_holes(mut self, holes: Vec<Arc<SPolygon>>) -> Self {
        self.holes = holes;
        self
    }

    /// Returns true if the candidate polygon is fully contained inside one of the
    /// hazard's safe inner regions (holes). When this is the case, the candidate
    /// should be considered non-colliding with this hazard.
    ///
    /// Performs (in order, with cheap early-outs):
    /// 1. Bounding-box enclosure test (`hole.bbox` surrounds `candidate.bbox`).
    /// 2. Point-in-polygon test (candidate's pole-of-inaccessibility center inside hole).
    /// 3. Edge-vs-edge non-intersection test.
    #[must_use]
    pub fn is_candidate_safely_in_hole(&self, candidate: &SPolygon) -> bool {
        if self.holes.is_empty() {
            return false;
        }
        for hole in &self.holes {
            // (1) bbox must surround
            if hole.bbox.relation_to(candidate.bbox) != GeoRelation::Surrounding {
                continue;
            }
            // (2) candidate's POI center must be inside hole
            if !hole.collides_with(&candidate.poi.center) {
                continue;
            }
            // (3) no edges may cross
            let mut crosses = false;
            'outer: for c_edge in candidate.edge_iter() {
                for h_edge in hole.edge_iter() {
                    if c_edge.collides_with(&h_edge) {
                        crosses = true;
                        break 'outer;
                    }
                }
            }
            if !crosses {
                return true;
            }
        }
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Entity inducing a [`Hazard`].
/// All entities are uniquely identified.
pub enum HazardEntity {
    /// An item placed in the layout, defined by its id, applied transformation and key
    PlacedItem {
        id: usize,
        dt: DTransformation,
        pk: PItemKey,
    },
    /// Represents all regions outside the container
    Exterior,
    /// Represents a hole in the container.
    Hole { idx: usize },
    /// Represents a zone in the container with a specific quality level that is inferior to the base quality.
    InferiorQualityZone { quality: usize, idx: usize },
}

impl HazardEntity {
    /// Whether the entity induced a hazard within the entire interior or exterior of its shape
    #[must_use]
    pub fn scope(&self) -> GeoPosition {
        match self {
            HazardEntity::PlacedItem { .. }
            | HazardEntity::Hole { .. }
            | HazardEntity::InferiorQualityZone { .. } => GeoPosition::Interior,
            HazardEntity::Exterior => GeoPosition::Exterior,
        }
    }
}

impl<T> From<(PItemKey, T)> for HazardEntity
where
    T: Borrow<PlacedItem>,
{
    fn from((pk, pi): (PItemKey, T)) -> Self {
        HazardEntity::PlacedItem {
            id: pi.borrow().item_id,
            dt: pi.borrow().d_transf,
            pk,
        }
    }
}
