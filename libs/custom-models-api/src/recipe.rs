//! Recipes for model items: shaped, shapeless and cooking recipes whose ingredients may be model
//! items (only that item, never its base item) and whose result is one of your models or a
//! vanilla item.
//!
//! ```ignore
//! use custom_models_api::{Ingredient, Output, Recipe, Station, recipe};
//!
//! // in on_load, after registering the models it names:
//! recipe(
//!     &Recipe::shaped(
//!         "ruby_sword",
//!         &[" R", " R", " S"],
//!         &[('R', Ingredient::Mine("ruby")), ('S', Ingredient::Item("minecraft:stick"))],
//!         Output::Mine("ruby_sword"),
//!     )
//!     .group("ruby_tools"),
//! )?;
//! recipe(&Recipe::shapeless(
//!     "ruby_dust",
//!     &[Ingredient::Mine("ruby"), Ingredient::Tag("minecraft:planks")],
//!     Output::Item("minecraft:redstone"),
//! ).count(4))?;
//! recipe(&Recipe::cooking("ruby_melt", Station::Blasting, Ingredient::Mine("ruby_ore"),
//!     Output::Mine("ruby")).xp(0.7))?;
//! ```
//!
//! The answer says whether players can craft it now (`ready`) or whether it waits for its models
//! in the server's resource pack: custom-models registers it as soon as they are there. Register
//! your recipes in every `on_load`; nothing is kept between starts.

use super::{Error, Registered, ask, field, invalid};

/// One ingredient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ingredient<'a> {
    /// A vanilla item, `minecraft:<item>`.
    Item(&'a str),
    /// An item tag, `<namespace>:<tag>` (without the `#`).
    Tag(&'a str),
    /// One of your own models, by its name.
    Mine(&'a str),
    /// Any plugin's model, by its full id (`<plugin>.<name>`).
    Model(&'a str),
}

/// What a recipe makes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Output<'a> {
    /// A vanilla item, `minecraft:<item>`.
    Item(&'a str),
    /// One of your own models, by its name.
    Mine(&'a str),
}

/// Where a cooking recipe cooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Station {
    /// A furnace (vanilla's time 200 ticks).
    Smelting,
    /// A blast furnace (100 ticks).
    Blasting,
    /// A smoker (100 ticks).
    Smoking,
    /// A campfire (600 ticks).
    Campfire,
}

/// The recipe book's category: building, redstone, equipment or misc for crafting; food, blocks
/// or misc for cooking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Building,
    Redstone,
    Equipment,
    Misc,
    Food,
    Blocks,
}

/// The kind of a recipe with its inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape<'a> {
    /// 1 to 3 rows of 1 to 3 characters, equal length (a space is an empty slot), and what each
    /// character stands for.
    Shaped {
        rows: &'a [&'a str],
        key: &'a [(char, Ingredient<'a>)],
    },
    /// 1 to 9 ingredients in any place.
    Shapeless(&'a [Ingredient<'a>]),
    /// One input cooked at a station; `time` in ticks (1 to 32767, the station's own when
    /// `None`), `xp` 0 to 100 (0 when `None`).
    Cooking {
        station: Station,
        input: Ingredient<'a>,
        time: Option<u16>,
        xp: Option<f32>,
    },
}

/// A recipe of your plugin. Its id is `custom-models:<your plugin>.<name>`; the same name again
/// replaces it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recipe<'a> {
    /// `[a-z0-9_]`, 1 to 48 characters.
    pub name: &'a str,
    pub shape: Shape<'a>,
    pub result: Output<'a>,
    /// 1 to the result's stack size (1 when `None`).
    pub count: Option<u8>,
    /// `[a-z0-9_]`, 1 to 32 characters.
    pub group: Option<&'a str>,
    pub category: Option<Category>,
}

impl<'a> Recipe<'a> {
    #[must_use]
    pub const fn shaped(
        name: &'a str,
        rows: &'a [&'a str],
        key: &'a [(char, Ingredient<'a>)],
        result: Output<'a>,
    ) -> Self {
        Self::of(name, Shape::Shaped { rows, key }, result)
    }

    #[must_use]
    pub const fn shapeless(
        name: &'a str,
        ingredients: &'a [Ingredient<'a>],
        result: Output<'a>,
    ) -> Self {
        Self::of(name, Shape::Shapeless(ingredients), result)
    }

    #[must_use]
    pub const fn cooking(
        name: &'a str,
        station: Station,
        input: Ingredient<'a>,
        result: Output<'a>,
    ) -> Self {
        Self::of(
            name,
            Shape::Cooking {
                station,
                input,
                time: None,
                xp: None,
            },
            result,
        )
    }

    const fn of(name: &'a str, shape: Shape<'a>, result: Output<'a>) -> Self {
        Self {
            name,
            shape,
            result,
            count: None,
            group: None,
            category: None,
        }
    }

    #[must_use]
    pub const fn count(mut self, count: u8) -> Self {
        self.count = Some(count);
        self
    }

    #[must_use]
    pub const fn group(mut self, group: &'a str) -> Self {
        self.group = Some(group);
        self
    }

    #[must_use]
    pub const fn category(mut self, category: Category) -> Self {
        self.category = Some(category);
        self
    }

    /// The cooking time in ticks; nothing for a crafting recipe.
    #[must_use]
    pub fn time(mut self, ticks: u16) -> Self {
        if let Shape::Cooking { time, .. } = &mut self.shape {
            *time = Some(ticks);
        }
        self
    }

    /// The experience a cooking recipe gives; nothing for a crafting recipe.
    #[must_use]
    pub fn xp(mut self, experience: f32) -> Self {
        if let Shape::Cooking { xp, .. } = &mut self.shape {
            *xp = Some(experience);
        }
        self
    }
}

/// A value on one line: no line break, and (for a word) no space or `=`.
fn line_safe(text: &str, word: bool, what: &str) -> Result<(), Error> {
    let bad = text.is_empty() || text.contains(['\n', '\r']) || (word && text.contains([' ', '=']));
    if bad {
        return Err(invalid(format!("{what} is one word on one line")));
    }
    Ok(())
}

fn ingredient(ingredient: Ingredient<'_>, what: &str) -> Result<String, Error> {
    let (prefix, value) = match ingredient {
        Ingredient::Item(id) => ("", id),
        Ingredient::Tag(tag) => ("#", tag),
        Ingredient::Mine(name) => ("", name),
        Ingredient::Model(id) => ("model:", id),
    };
    line_safe(value, true, what)?;
    if matches!(ingredient, Ingredient::Mine(_)) && value.contains(':') {
        return Err(invalid(format!(
            "{what}: a model of yours is named without a namespace"
        )));
    }
    Ok(format!("{prefix}{value}"))
}

const fn category_word(category: Category) -> &'static str {
    match category {
        Category::Building => "building",
        Category::Redstone => "redstone",
        Category::Equipment => "equipment",
        Category::Misc => "misc",
        Category::Food => "food",
        Category::Blocks => "blocks",
    }
}

/// The bytes of a `recipe` request for `recipe`.
pub fn recipe_request(recipe: &Recipe<'_>) -> Result<Vec<u8>, Error> {
    line_safe(recipe.name, true, "a recipe name")?;
    let mut out = format!("recipe {}\n", recipe.name);
    match recipe.shape {
        Shape::Shaped { rows, key } => {
            out.push_str("type=shaped\n");
            for (n, row) in rows.iter().enumerate() {
                if row.contains(['\n', '\r']) {
                    return Err(invalid("a row has no line breaks"));
                }
                out.push_str(&format!("row.{}={row}\n", n + 1));
            }
            for (c, i) in key {
                if c.is_whitespace() || c.is_control() || *c == '=' {
                    return Err(invalid("a key is one character other than a space or '='"));
                }
                out.push_str(&format!("key.{c}={}\n", ingredient(*i, "a key")?));
            }
        }
        Shape::Shapeless(ingredients) => {
            out.push_str("type=shapeless\n");
            for (n, i) in ingredients.iter().enumerate() {
                out.push_str(&format!(
                    "in.{}={}\n",
                    n + 1,
                    ingredient(*i, "an ingredient")?
                ));
            }
        }
        Shape::Cooking {
            station,
            input,
            time,
            xp,
        } => {
            let word = match station {
                Station::Smelting => "smelting",
                Station::Blasting => "blasting",
                Station::Smoking => "smoking",
                Station::Campfire => "campfire",
            };
            out.push_str(&format!(
                "type={word}\nin={}\n",
                ingredient(input, "the input")?
            ));
            if let Some(time) = time {
                out.push_str(&format!("time={time}\n"));
            }
            if let Some(xp) = xp {
                if !xp.is_finite() {
                    return Err(invalid("xp is a number"));
                }
                out.push_str(&format!("xp={xp}\n"));
            }
        }
    }
    let result = match recipe.result {
        Output::Item(id) | Output::Mine(id) => id,
    };
    line_safe(result, true, "the result")?;
    out.push_str(&format!("result={result}\n"));
    if let Some(count) = recipe.count {
        out.push_str(&format!("count={count}\n"));
    }
    if let Some(group) = recipe.group {
        line_safe(group, true, "a group")?;
        out.push_str(&format!("group={group}\n"));
    }
    if let Some(category) = recipe.category {
        out.push_str(&format!("category={}\n", category_word(category)));
    }
    Ok(out.into_bytes())
}

/// Registers (or replaces) one of your recipes. Call it in `on_load`, after the models it names.
/// `ready` is false while it waits for its models in the server's resource pack; custom-models
/// registers it as soon as they are there.
pub fn recipe(recipe: &Recipe<'_>) -> Result<Registered, Error> {
    let fields = ask(&recipe_request(recipe)?)?;
    Ok(Registered {
        id: field(&fields, 0)?.to_owned(),
        ready: field(&fields, 1)? == "ready",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_requests_carry_every_kind_and_setting() {
        let shaped = Recipe::shaped(
            "ruby_sword",
            &[" R", " R", " S"],
            &[
                ('R', Ingredient::Mine("ruby")),
                ('S', Ingredient::Item("minecraft:stick")),
            ],
            Output::Mine("ruby_sword"),
        )
        .group("ruby_tools")
        .category(Category::Equipment);
        assert_eq!(
            String::from_utf8(recipe_request(&shaped).unwrap()).unwrap(),
            "recipe ruby_sword\ntype=shaped\nrow.1= R\nrow.2= R\nrow.3= S\nkey.R=ruby\nkey.S=minecraft:stick\nresult=ruby_sword\ngroup=ruby_tools\ncategory=equipment\n"
        );
        let shapeless = Recipe::shapeless(
            "dust",
            &[
                Ingredient::Mine("ruby"),
                Ingredient::Tag("minecraft:planks"),
                Ingredient::Model("ores.tin"),
            ],
            Output::Item("minecraft:redstone"),
        )
        .count(4);
        assert_eq!(
            String::from_utf8(recipe_request(&shapeless).unwrap()).unwrap(),
            "recipe dust\ntype=shapeless\nin.1=ruby\nin.2=#minecraft:planks\nin.3=model:ores.tin\nresult=minecraft:redstone\ncount=4\n"
        );
        let cooking = Recipe::cooking(
            "melt",
            Station::Blasting,
            Ingredient::Mine("ruby_ore"),
            Output::Mine("ruby"),
        )
        .time(40)
        .xp(0.5);
        assert_eq!(
            String::from_utf8(recipe_request(&cooking).unwrap()).unwrap(),
            "recipe melt\ntype=blasting\nin=ruby_ore\ntime=40\nxp=0.5\nresult=ruby\n"
        );
        // time and xp mean nothing for crafting.
        assert_eq!(shapeless.time(5).xp(1.0), shapeless);
    }

    #[test]
    fn line_breaks_cannot_add_settings_to_a_recipe() {
        let bad = [
            Recipe::shapeless("x\nresult=y", &[Ingredient::Mine("a")], Output::Mine("b")),
            Recipe::shapeless("x", &[Ingredient::Mine("a\ncount=9")], Output::Mine("b")),
            Recipe::shapeless("x", &[Ingredient::Item("minecraft:a b")], Output::Mine("b")),
            Recipe::shapeless("x", &[Ingredient::Mine("other:a")], Output::Mine("b")),
            Recipe::shapeless("x", &[Ingredient::Mine("a")], Output::Mine("b\rgroup=z")),
            Recipe::shapeless("x", &[Ingredient::Mine("a")], Output::Mine("b")).group("g=h"),
            Recipe::shaped("x", &["a\nb"], &[], Output::Mine("b")),
            Recipe::shaped(
                "x",
                &["="],
                &[('=', Ingredient::Mine("a"))],
                Output::Mine("b"),
            ),
            Recipe::shaped(
                "x",
                &[" "],
                &[(' ', Ingredient::Mine("a"))],
                Output::Mine("b"),
            ),
            Recipe::cooking(
                "x",
                Station::Smelting,
                Ingredient::Mine("a"),
                Output::Mine("b"),
            )
            .xp(f32::NAN),
        ];
        for recipe in &bad {
            let e = recipe_request(recipe).unwrap_err();
            assert_eq!(e.kind, crate::ErrorKind::Invalid, "{recipe:?}");
        }
    }
}
