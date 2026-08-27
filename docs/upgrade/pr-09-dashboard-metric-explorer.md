# PR-09 Dashboard information architecture + Metric Explorer

Status: executable product contract for the post-PR-08 dashboard enhancement.

## Baseline audit

- `getAvailableMetricDescriptors(samples)` is a range-data helper, not a metric catalog. A missing value in the selected range must never remove a known metric from the product.
- Existing provider truth is authoritative for collection state: `CollectionSettings.enabledCategories` and `disabledProviders`, `CollectorStatus.providerStatus`, stable GPU `deviceKey`, and the current-session `collection_session_metric` metadata already persisted by the collector.
- Timeline DTOs contain bounded rendered samples and coverage/gaps. They do not provide authoritative dashboard aggregates for average, peak, energy, or previous-period delta.

## Information hierarchy

1. **Overview** — a compact adaptive set of current-value cards answers “what is happening now?” CPU, memory, disk I/O, and usable GPU utilization/temperature appear only when the capability is usable. Missing values show an explicit state; numeric zero remains visible as zero.
2. **Trends** — one prominent ECharts workspace with unit-family groups and metric chips. Only compatible metrics share a view; switching families changes the view rather than mixing unrelated axes.
3. **Metric Explorer** — a searchable, grouped catalog of known system and GPU metrics. The catalog is independent of the selected range and shows readable names, units, device identity, status, and pin/trend state.
4. **Detail** — existing bounded timeline selection, GPU device detail, provider health, and process evidence remain progressively disclosed below the primary workspace.

## Catalog and status semantics

The frontend projects provider metadata plus current-range samples into:

- `AVAILABLE`: supported/enabled and a finite numeric sample exists in range.
- `NO_DATA_IN_RANGE`: supported/enabled, but this range has no finite numeric sample.
- `DISABLED`: the relevant collection category/provider path is disabled; dashboard actions never enable collection.
- `UNSUPPORTED`: provider capability says the metric cannot be supplied.
- `FAILED`: expected capability has a failed probe/runtime/provider state.
- `DEGRADED`: valid capability/data remains while provider health reports a partial failure.

These states are never inferred from `value == null` or from `0`. Unknown is reserved for a genuinely unavailable classification.

## Defaults and drill-down

Defaults are conservative and adaptive: CPU, memory, and disk I/O are separate overview groups when usable; GPU utilization and temperature use at most two additional groups across all known devices. The full GPU field set remains in the explorer/detail view. No synthetic CPU sensors, comparison rankings, or aggregate claims are introduced.

## Customization and persistence

The primary actions are pin/unpin overview metrics, add/remove trend metrics, reorder overview groups, and restore adaptive defaults. Internal card IDs are not exposed. The existing `DashboardConfig` v1 shape remains because it already stores ordered compatible metric groups and visibility; trend selections are workspace-local and are not persisted. No cosmetic version bump is allowed.

Loading states stay distinct: a valid saved config, a successful empty result, and a failed read are separate. Failed reads show retry and never persist fallback defaults. Only a user change/save writes the existing `ui.dashboard.v1` key; unrelated settings are untouched. Malformed configs fail closed.

## Accessibility, performance, and non-goals

- Keyboard-focusable controls, visible focus, labelled search/toggles, text status badges (not color alone), and a concise accessible chart summary are required. Reduced motion remains respected.
- Timeline queries stay bounded; the explorer has no per-row charts; the trend workspace owns one active ECharts instance and updates it without normal-refresh reinitialization. No unbounded history or hidden-detail polling is added.
- Do not add providers, schema migrations, new chart frameworks, synthetic aggregates, benchmark/ranking claims, AI recommendations, or unrelated settings/sleep-wake changes.
