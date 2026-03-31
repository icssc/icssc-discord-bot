use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SocialTeamPairingRound::Table)
                    .if_not_exists()
                    .col(pk_auto(SocialTeamPairingRound::Id))
                    .col(timestamp(SocialTeamPairingRound::CreatedAt).default(Expr::cust("NOW()")))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SocialTeamPairingGroup::Table)
                    .if_not_exists()
                    .col(pk_auto(SocialTeamPairingGroup::Id))
                    .col(integer(SocialTeamPairingGroup::RoundId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(SocialTeamPairingGroup::Table, SocialTeamPairingGroup::RoundId)
                            .to(SocialTeamPairingRound::Table, SocialTeamPairingRound::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SocialTeamPairingEntry::Table)
                    .if_not_exists()
                    .col(integer(SocialTeamPairingEntry::GroupId))
                    .col(text(SocialTeamPairingEntry::TeamName))
                    .primary_key(
                        Index::create()
                            .col(SocialTeamPairingEntry::GroupId)
                            .col(SocialTeamPairingEntry::TeamName),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                SocialTeamPairingEntry::Table,
                                SocialTeamPairingEntry::GroupId,
                            )
                            .to(SocialTeamPairingGroup::Table, SocialTeamPairingGroup::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(SocialTeamPairingEntry::Table)
                    .cascade()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(SocialTeamPairingGroup::Table)
                    .cascade()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(SocialTeamPairingRound::Table)
                    .cascade()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum SocialTeamPairingRound {
    Table,
    Id,
    CreatedAt,
}

#[derive(DeriveIden)]
enum SocialTeamPairingGroup {
    Table,
    Id,
    RoundId,
}

#[derive(DeriveIden)]
enum SocialTeamPairingEntry {
    Table,
    GroupId,
    TeamName,
}
