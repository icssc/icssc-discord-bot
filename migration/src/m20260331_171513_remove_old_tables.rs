use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ServerEvent::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(ServerCalendar::Table).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ServerCalendar::Table)
                    .if_not_exists()
                    .col(big_integer(ServerCalendar::GuildId))
                    .col(text(ServerCalendar::CalendarId))
                    .col(text(ServerCalendar::CalendarName))
                    .col(text(ServerCalendar::WebhookId))
                    .col(text(ServerCalendar::AccessToken))
                    .col(timestamp(ServerCalendar::AccessExpires))
                    .col(text(ServerCalendar::RefreshToken))
                    .primary_key(
                        Index::create()
                            .col(ServerCalendar::GuildId)
                            .col(ServerCalendar::CalendarId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ServerEvent::Table)
                    .if_not_exists()
                    .col(big_integer(ServerEvent::GuildId))
                    .col(text(ServerEvent::CalendarId))
                    .col(text(ServerEvent::CalendarEventId))
                    .col(big_integer(ServerEvent::GuildEventId))
                    .primary_key(
                        Index::create()
                            .col(ServerEvent::GuildId)
                            .col(ServerEvent::CalendarId)
                            .col(ServerEvent::CalendarEventId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                ServerEvent::Table,
                                (ServerEvent::GuildId, ServerEvent::CalendarId),
                            )
                            .to(
                                ServerCalendar::Table,
                                (ServerCalendar::GuildId, ServerCalendar::CalendarId),
                            )
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ServerCalendar {
    Table,
    GuildId,
    CalendarId,
    CalendarName,
    AccessToken,
    AccessExpires,
    RefreshToken,
    WebhookId,
}

#[derive(DeriveIden)]
enum ServerEvent {
    Table,
    GuildId,
    CalendarId,
    CalendarEventId,
    GuildEventId,
}
