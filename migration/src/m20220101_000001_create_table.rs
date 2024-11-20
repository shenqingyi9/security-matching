use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{
    ActiveEnum, DbBackend, DeriveActiveEnum, EnumIter, Iterable, Schema,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Ac::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Ac::Id)
                            .big_integer()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(Ac::Pwd).text().not_null())
                    .col(ColumnDef::new(Ac::Name).string().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Security::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Security::Code).string().primary_key())
                    .col(ColumnDef::new(Security::Name).string().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Req::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Req::Seq)
                            .big_integer()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(Req::Id).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Req::Table, Req::Id)
                            .to(Ac::Table, Ac::Id),
                    )
                    .col(ColumnDef::new(Req::Body).json().not_null())
                    .col(
                        ColumnDef::new(Req::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_type(Schema::new(DbBackend::Postgres).create_enum_from_active_enum::<Dir>())
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Order::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Order::Seq).big_integer().primary_key())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Order::Table, Order::Seq)
                            .to(Req::Table, Req::Seq),
                    )
                    .col(ColumnDef::new(Order::Code).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Order::Table, Order::Code)
                            .to(Security::Table, Security::Code),
                    )
                    .col(
                        ColumnDef::new(Order::Dir)
                            .enumeration(Dir::name(), Dir::iter())
                            .not_null(),
                    )
                    .col(ColumnDef::new(Order::Price).decimal_len(1000, 2).not_null())
                    .col(ColumnDef::new(Order::Quantity).big_integer().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Rec::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Rec::Ack)
                            .big_integer()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(Rec::Code).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Rec::Table, Rec::Code)
                            .to(Security::Table, Security::Code),
                    )
                    .col(ColumnDef::new(Rec::BuyerId).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Rec::Table, Rec::BuyerId)
                            .to(Ac::Table, Ac::Id),
                    )
                    .col(ColumnDef::new(Rec::SellerId).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Rec::Table, Rec::SellerId)
                            .to(Ac::Table, Ac::Id),
                    )
                    .col(ColumnDef::new(Rec::Price).decimal_len(1000, 2).not_null())
                    .col(ColumnDef::new(Rec::Quantity).big_integer().not_null())
                    .col(
                        ColumnDef::new(Rec::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Msg::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Msg::Ack)
                            .big_integer()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(Msg::Id).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Msg::Table, Msg::Id)
                            .to(Ac::Table, Ac::Id),
                    )
                    .col(ColumnDef::new(Msg::EventType).string().not_null())
                    .col(ColumnDef::new(Msg::Data).json().not_null())
                    .col(
                        ColumnDef::new(Msg::HappenedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Msg::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Rec::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Order::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Req::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Security::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Ac::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Ac {
    Table,
    Id,
    Pwd,
    Name,
}

#[derive(DeriveIden)]
enum Security {
    Table,
    Code,
    Name,
}

#[derive(DeriveIden)]
enum Req {
    Table,
    Seq,
    Id,
    Body,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Order {
    Table,
    Seq,
    Code,
    Dir,
    Price,
    Quantity,
}

#[derive(DeriveIden, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "dir")]
enum Dir {
    #[sea_orm(string_value = "Buy")]
    Buy,
    #[sea_orm(string_value = "Sell")]
    Sell,
}

#[derive(DeriveIden)]
enum Rec {
    Table,
    Ack,
    Code,
    BuyerId,
    SellerId,
    Price,
    Quantity,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Msg {
    Table,
    Ack,
    Id,
    EventType,
    Data,
    HappenedAt,
}
