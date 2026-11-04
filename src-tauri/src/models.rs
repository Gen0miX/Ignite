//! Core domain types shared by the database layer and Tauri commands.

use serde::{Deserialize, Serialize};

macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text),+ }
            }
        }

        impl std::str::FromStr for $name {
            type Err = crate::error::AppError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok($name::$variant),)+
                    other => Err(crate::error::AppError::Invalid(
                        format!("unknown {} '{}'", stringify!($name), other),
                    )),
                }
            }
        }

        impl rusqlite::types::ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(self.as_str().into())
            }
        }

        impl rusqlite::types::FromSql for $name {
            fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
                v.as_str()?
                    .parse()
                    .map_err(|e: crate::error::AppError| rusqlite::types::FromSqlError::Other(e.into()))
            }
        }
    };
}

string_enum!(
    /// User-chosen progress status of a game.
    GameStatus { Todo => "todo", Playing => "playing", Done => "done", Dropped => "dropped" }
);

string_enum!(
    /// Platform a copy of a game comes from.
    SourceKind {
        Steam => "steam",
        Epic => "epic",
        Gog => "gog",
        Battlenet => "battlenet",
        Ea => "ea",
        Ubisoft => "ubisoft",
        Xbox => "xbox",
        Riot => "riot",
        Local => "local",
        Emulator => "emulator",
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_round_trip_through_strings() {
        for s in GameStatus::ALL {
            assert_eq!(s.as_str().parse::<GameStatus>().unwrap(), *s);
        }
        for k in SourceKind::ALL {
            assert_eq!(k.as_str().parse::<SourceKind>().unwrap(), *k);
        }
        assert!("steamy".parse::<SourceKind>().is_err());
    }

    #[test]
    fn serde_uses_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&SourceKind::Battlenet).unwrap(),
            "\"battlenet\""
        );
        assert_eq!(
            serde_json::from_str::<GameStatus>("\"dropped\"").unwrap(),
            GameStatus::Dropped
        );
    }
}
