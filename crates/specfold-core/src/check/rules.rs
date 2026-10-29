//! The rule catalog, one module per family.

mod asset_refs;
mod assets;
mod attachments;
mod cites;
mod commands;
mod counts;
mod forbid;
mod guarded;
mod ids;
mod lifecycle;
mod links;
mod placement;
mod schema;
pub use schema::unknown_key;
mod support;
mod visibility;

use super::rule::Rule;

pub fn catalog() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(schema::Frontmatter),
        Box::new(schema::IdMatchesPath),
        Box::new(schema::Placement),
        Box::new(schema::StatusAllowed),
        Box::new(schema::Dates),
        Box::new(schema::RequiredByStatus),
        Box::new(ids::UniqueIds),
        Box::new(ids::AliasesClean),
        Box::new(ids::AliasTableFrozen),
        Box::new(links::LinksResolve),
        Box::new(links::NoPathCites),
        Box::new(links::RelationsResolve),
        Box::new(links::AnchorsUnique),
        Box::new(links::GovernsExists),
        Box::new(cites::CitesOutside),
        Box::new(lifecycle::SupersedePairs),
        Box::new(lifecycle::ForwardOnly),
        Box::new(lifecycle::Sealed),
        Box::new(lifecycle::UpdatedMoves),
        Box::new(visibility::NotInPublic),
        Box::new(visibility::NoLocalCites),
        Box::new(visibility::ExternalCites),
        Box::new(placement::OwnerMatches),
        Box::new(attachments::Registered),
        Box::new(attachments::SizeLimit),
        Box::new(attachments::HashesMatch),
        Box::new(attachments::StoredExist),
        Box::new(forbid::Forbidden),
        Box::new(commands::VerifyPasses),
        Box::new(commands::VerifyDeclared),
        Box::new(commands::GeneratedInSync),
        Box::new(guarded::GuardsPass),
        Box::new(assets::AssetFrontmatter),
        Box::new(assets::AssetNamesUnique),
        Box::new(asset_refs::AssetRefsExist),
        Box::new(counts::CountsMatch),
    ]
}
