use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Message::Table)
                    .if_not_exists()
                    .col(big_integer(Message::GuildId))
                    .col(big_integer(Message::ChannelId))
                    .col(big_integer(Message::MessageId))
                    .col(big_integer(Message::AuthorId))
                    .col(timestamp(Message::TimePosted).generated(
                        Expr::cust("to_timestamp((message_id / (2 ^ 22) + 1420070400000) / 1000)"),
                        true,
                    ))
                    .primary_key(Index::create().col(Message::MessageId))
                    .to_owned(),
            )
            .await?;

        let lat_null = Snipe::Latitude.into_column_ref().is_null();
        let lon_null = Snipe::Longitude.into_column_ref().is_null();

        manager
            .create_table(
                Table::create()
                    .table(Snipe::Table)
                    .if_not_exists()
                    .col(big_integer(Snipe::MessageId))
                    .col(big_integer(Snipe::VictimId))
                    .col(float_null(Snipe::Latitude))
                    .col(float_null(Snipe::Longitude))
                    .col(string_null(Snipe::Notes))
                    .primary_key(Index::create().col(Snipe::MessageId).col(Snipe::VictimId))
                    .check(SimpleExpr::from(
                        Condition::any()
                            .add(
                                lat_null
                                    .clone()
                                    .and(lon_null.clone())
                                    .or(lat_null.not().and(lon_null.not())),
                            )
                            .to_owned(),
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .from(Snipe::Table, Snipe::MessageId)
                    .to(Message::Table, Message::MessageId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OptOut::Table)
                    .if_not_exists()
                    .col(big_integer(OptOut::Id))
                    .primary_key(Index::create().col(OptOut::Id))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE MATERIALIZED VIEW IF NOT EXISTS user_stat AS
SELECT u.id,
       COALESCE(snipe.cnt, 0)::bigint  AS snipe,
       COALESCE(sniped.cnt, 0)::bigint AS sniped,
       CASE
           WHEN COALESCE(sniped.cnt, 0) = 0 THEN NULL
           ELSE CAST(COALESCE(snipe.cnt, 0) AS DOUBLE PRECISION) / COALESCE(sniped.cnt, 0)
           END                         AS snipe_rate
FROM (SELECT DISTINCT author_id AS id
      FROM message
      UNION
      SELECT DISTINCT victim_id
      FROM snipe) u
         LEFT JOIN
     (SELECT author_id, COUNT(*) AS cnt
      FROM message
               LEFT JOIN snipe on message.message_id = snipe.message_id
      GROUP BY author_id) snipe
     ON u.id = snipe.author_id
         LEFT JOIN
         (SELECT victim_id, COUNT(*) AS cnt FROM snipe GROUP BY victim_id) sniped
         ON u.id = sniped.victim_id
ORDER BY snipe_rate DESC NULLS LAST;

REFRESH MATERIALIZED VIEW user_stat;
        "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP MATERIALIZED VIEW user_stat;")
            .await?;

        manager
            .drop_table(Table::drop().table(OptOut::Table).cascade().to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Snipe::Table).cascade().to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Message::Table).cascade().to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Message {
    Table,
    GuildId,
    ChannelId,
    #[allow(clippy::enum_variant_names)]
    MessageId,
    AuthorId,
    TimePosted,
}

#[derive(DeriveIden)]
enum Snipe {
    Table,
    MessageId,
    VictimId,
    Latitude,
    Longitude,
    Notes,
}

#[derive(DeriveIden)]
enum OptOut {
    Table,
    Id,
}
