pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20250905_213900_matchy_history;
mod m20250916_174534_social_spottings;
mod m20250923_231905_matchy_opt_in;
mod m20250925_083518_matchy_pair_cols;
mod m20250926_214250_stats_with_socials;
mod m20250930_012439_calendar_events;
mod m20251003_231509_calendar_indexes;
mod m20251013_031245_message_on_delete_cascade;
mod m20260112_055632_modernize_table_names;
mod m20260223_234418_change_social_multiplier_mview;
mod m20260331_171513_remove_old_tables;
mod m20260331_171625_create_teampair_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20250905_213900_matchy_history::Migration),
            Box::new(m20250916_174534_social_spottings::Migration),
            Box::new(m20250923_231905_matchy_opt_in::Migration),
            Box::new(m20250925_083518_matchy_pair_cols::Migration),
            Box::new(m20250926_214250_stats_with_socials::Migration),
            Box::new(m20250930_012439_calendar_events::Migration),
            Box::new(m20251003_231509_calendar_indexes::Migration),
            Box::new(m20251013_031245_message_on_delete_cascade::Migration),
            Box::new(m20260112_055632_modernize_table_names::Migration),
            Box::new(m20260223_234418_change_social_multiplier_mview::Migration),
            Box::new(m20260331_171513_remove_old_tables::Migration),
            Box::new(m20260331_171625_create_teampair_table::Migration),
        ]
    }
}
