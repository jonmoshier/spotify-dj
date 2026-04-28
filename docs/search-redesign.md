# Search Redesign

## Background

Spotify's track search API supports field filter syntax natively. Since spotify-dj passes the query string through verbatim, these filters already work — but the current single-line text input gives no affordance for them and requires retyping every time.

Supported filters:
- `genre:techno`
- `year:2020` or `year:2018-2023`
- `artist:bicep`
- `album:isles`
- `tag:new` — tracks added in the last two weeks
- `tag:hipster` — lower-popularity tracks
- Combinable freely: `genre:house year:2019-2023`

The redesign treats genre and year as first-class persistent state rather than typed syntax.

---

## Problem with the Current UX

A DJ mid-set thinks in constraints, not queries. They need something in a specific genre and era that fits the current vibe. The current flow:

1. Press `/`
2. Type `genre:house year:2019-2023 bicep`
3. Browse results
4. Clear and type `genre:house year:2019-2023 floating points`
5. Repeat — retyping the filters every single time

This breaks flow. Filters should be set once and persist across searches.

---

## Proposed Layout

The library panel gains a second row between the search input and the results list:

```
╭─ Library ──────────────────────────────────╮
│ / bicep                                    │  freetext (title / artist)
│ genre:[house          ] year:[2019–2023]   │  persistent filter row
├────────────────────────────────────────────┤
│ ▶ Feed Me — Bicep                          │
│   Glue — Bicep                             │
│   Sundial — Bicep                          │
│   ...                                      │
╰────────────────────────────────────────────╯
```

The filter row takes one line when idle (showing set values) and one line when active (showing a cursor in the focused field). Total cost: one row of vertical space in the library panel.

When no filters are set the row shows a dim hint:

```
│ G genre  Y year                            │
```

When filters are set it shows them compact:

```
│ genre:house  year:2019-2023  [Ctrl+X clear]│
```

---

## Key Bindings

All bindings active while Library panel has focus.

| Key | Action |
|---|---|
| `/` | Enter freetext input mode |
| `G` | Enter genre filter edit mode |
| `Y` | Enter year filter edit mode |
| `Tab` | Cycle focus: freetext → genre → year → freetext |
| `Enter` | Run search (from any field) — combines all active filters |
| `Ctrl+G` | Clear genre filter |
| `Ctrl+Y` | Clear year filter |
| `Ctrl+X` | Clear all filters and freetext |
| `Esc` | Exit edit mode, keep current values |
| `↑ ↓` / scroll | Navigate results (when not in edit mode) |
| `L` / `R` | Load selected track → Deck A / Deck B |

Pressing `Enter` with only filters set and no freetext is valid — runs a genre/year-only search.

---

## Filter Persistence

Filters survive across searches for the duration of the session. They are **not** persisted to disk — on next launch the filter row starts empty. This avoids the confusion of launching the app into a stale filter state.

Future: could optionally save the last-used filter set to the track DB.

---

## Config-Defined Presets

Up to 5 filter presets defined in `config.toml`. Number keys `1`–`5` load a preset instantly while the Library panel is focused, then fire a search using any existing freetext.

```toml
[[ui.search_presets]]
name = "Late night techno"
genre = "techno"
year = "2018-2023"

[[ui.search_presets]]
name = "Classic house"
genre = "house"
year = "1988-1998"

[[ui.search_presets]]
name = "New releases"
tag = "new"
```

Pressing `1` sets genre and year from the preset, shows them in the filter row, and immediately fires a search. The freetext field is left as-is so you can combine: type "bicep", press `2` to switch to Classic house — results update to bicep tracks from 1988–1998.

Presets are displayed as a hint in the filter row when idle:

```
│ [1] Late night techno  [2] Classic house  [3] New releases  │
```

---

## BPM Reference Display

Since BPM is detected locally from the active deck, it cannot be used as a Spotify search filter. Instead, show it as a passive reference below the results so the DJ knows what they're matching to:

```
│ Active deck: ~128 BPM                      │
```

This line only appears when the active deck is playing and a BPM has been detected. It has no interactive function.

---

## Query Construction

The search query sent to Spotify is built by concatenating active components:

```
{freetext} {genre filter} {year filter} {tag filter}
```

Examples:

| Freetext | Genre | Year | Query sent |
|---|---|---|---|
| `bicep` | `house` | `2019-2023` | `bicep genre:house year:2019-2023` |
| _(empty)_ | `techno` | `2020` | `genre:techno year:2020` |
| `four tet` | _(none)_ | `2010-2015` | `four tet year:2010-2015` |
| _(empty)_ | _(none)_ | _(none)_ | _(no search fired)_ |

---

## State Changes

New fields on `LibraryState`:

```rust
pub struct LibraryState {
    pub search_query: String,       // freetext — existing
    pub filter_genre: String,       // new
    pub filter_year: String,        // new
    pub filter_tag: String,         // new — "new" or "hipster"
    pub search_focus: SearchFocus,  // new — which field has cursor
    pub results: Vec<TrackSummary>, // existing
    pub selected: usize,            // existing
    pub is_searching: bool,         // existing — true when any field is active
}

pub enum SearchFocus {
    None,
    Freetext,
    Genre,
    Year,
}
```

---

## Build Order

1. **Persistent genre + year filters** — `G`/`Y` keys, `Enter` to search, `Esc` to exit field, `Ctrl+G`/`Ctrl+Y` to clear. UI row with compact display when set.
2. **`Ctrl+X` clear all** and re-run empty search.
3. **Config presets** — parse `[[ui.search_presets]]`, number key bindings, preset hint display.
4. **BPM reference line** — passive display below results when active deck BPM is known.

---

## Open Questions

- Should year accept freeform input (the user types `2019-2023`) or two separate from/to fields? Freeform is simpler but allows malformed input. Two fields are cleaner but take more horizontal space.
- Should presets replace or merge with existing filters? Current proposal: replace (preset sets genre+year, overwriting whatever was there). Merge would be more flexible but harder to reason about.
- Should `Enter` from the results list (not a search field) re-run the last search, or do nothing? Currently `Enter` has no binding in the results list.
