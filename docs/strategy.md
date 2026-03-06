# School Run Strategy Guide

This document explains the School Run trading strategy as implemented in the `sr` backtesting engine. It is based on Tom Hougaard's *School Run Strategy* (December 2022), a mechanical breakout strategy that exploits the transition from market-maker-driven price action to genuine directional conviction after market open.

---

## What is the School Run Strategy?

Institutional market makers receive overnight and pre-market orders from clients that must be executed at the session open. This creates noisy, often misleading price action during the first 15 minutes of trading. The second 15-minute candle after market open captures the transition from this order-driven noise to real directional conviction.

The School Run Strategy identifies this second candle (the "signal bar") and places breakout orders above its high and below its low. When price breaks out of the signal bar's range, it signals the emerging trend for the session.

The strategy is purely mechanical -- no discretionary chart reading is required for entry or stop placement. Exit rules are parameterized for backtesting since Hougaard's original approach uses discretionary exits.

---

## Signal Bar Identification

The signal bar is the 2nd 15-minute candle after the exchange session opens. Its exact UTC time depends on the instrument and whether daylight saving time is in effect.

### Signal Bar Times by Instrument

| Instrument | Exchange | Session Open (Local) | Signal Bar (Local) | Winter UTC | Summer UTC |
|---|---|---|---|---|---|
| DAX 40 | XETRA, Frankfurt | 09:00 CET | 09:15 - 09:30 CET | 08:15 UTC | 07:15 UTC |
| FTSE 100 | LSE, London | 08:00 GMT | 08:15 - 08:30 GMT | 08:15 UTC | 07:15 UTC |
| Nasdaq | NYSE/NASDAQ, New York | 09:30 EST | 09:45 - 10:00 EST | 14:45 UTC | 13:45 UTC |
| Dow Jones | NYSE, New York | 09:30 EST | 09:45 - 10:00 EST | 14:45 UTC | 13:45 UTC |

DST transitions shift the UTC time by exactly one hour. The engine uses the IANA timezone database (`chrono-tz`) to compute the correct UTC offset for any date, handling all DST edge cases automatically.

The `signal_bar_index` parameter (default: 2) controls which candle is used. Index 1 would use the first candle after open; index 3 would use the third. Hougaard uses 2 for all instruments.

### Implementation

Signal bar detection is implemented in `engine/src/strategy/signal.rs`:

```rust
pub fn find_signal_bar(
    candles: &[Candle],
    instrument: Instrument,
    date: NaiveDate,
    config: &StrategyConfig,
) -> Option<SignalBar>
```

The function computes the expected signal bar timestamp via `instrument.signal_bar_start_utc(date)`, then finds the matching candle. Returns `None` on holidays or data gaps.

---

## Entry Mechanism

Once the signal bar is identified, two stop orders are placed:

- **Buy stop**: `signal_bar.high + entry_offset_points` (default: 2 points above)
- **Sell stop**: `signal_bar.low - entry_offset_points` (default: 2 points below)

Both orders remain active for the entire session by default. When `allow_both_sides` is `true` (default), both the long and short can trigger in the same session.

### Worked Example: DAX Signal Bar

Suppose the DAX signal bar on 15 January 2024 (winter, CET) has:

```
Candle: 09:15-09:30 CET (08:15 UTC)
  Open:  16,000.50
  High:  16,050.00
  Low:   15,980.00
  Close: 16,030.00
```

With the default `entry_offset_points = 2`:

- **Buy stop** placed at `16,050 + 2 = 16,052`
- **Sell stop** placed at `15,980 - 2 = 15,978`

If the DAX rises to 16,052 later in the session, the buy stop triggers and a long position is opened.

### Fill Simulation

Orders are filled against subsequent 15-minute candles:

- **Buy stop fills** when `candle.high >= trigger_price`. Fill price is `max(trigger_price, candle.open)` to handle gap-up opens.
- **Sell stop fills** when `candle.low <= trigger_price`. Fill price is `min(trigger_price, candle.open)` to handle gap-down opens.
- **Both sides triggered on same candle**: the order closest to `candle.open` fills first. Ties are broken in favor of the buy order.
- **Slippage**: configurable per fill (default: 0.5 points). Added for longs, subtracted for shorts.

---

## Stop Loss Modes

Three stop loss modes are supported, each computing the initial stop differently.

### 1. Signal Bar Extreme (`SignalBarExtreme`)

The stop is placed at the opposite extreme of the signal bar.

- **Long position**: stop at signal bar low
- **Short position**: stop at signal bar high

This mode produces variable stop distances depending on the signal bar's range. Wide signal bars produce large stops.

**Example** (DAX signal bar: high = 16,050, low = 15,980):

| Direction | Entry | Stop Loss | Risk |
|---|---|---|---|
| Long | 16,052 | 15,980 | 72 points |
| Short | 15,978 | 16,050 | 72 points |

### 2. Fixed Points (`FixedPoints`)

A fixed stop distance in points from the entry price.

- **Long position**: stop at `entry - sl_fixed_points`
- **Short position**: stop at `entry + sl_fixed_points`

Default: 40 points (calibrated for DAX near 12,000). When `sl_scale_with_index` is enabled, the distance scales proportionally:

```
scaled_sl = sl_fixed_points * (current_price / sl_scale_baseline)
```

**Example** (DAX at 18,000 with scaling, baseline 12,000):

```
Scaled SL = 40 * (18,000 / 12,000) = 60 points
Long entry at 16,052 -> Stop at 15,992
Short entry at 15,978 -> Stop at 16,038
```

**Example** (DAX without scaling):

| Direction | Entry | Stop Loss | Risk |
|---|---|---|---|
| Long | 16,052 | 16,012 | 40 points |
| Short | 15,978 | 16,018 | 40 points |

### 3. Midpoint (`Midpoint`)

The stop is placed at the midpoint of the signal bar's range, with an optional offset buffer.

```
midpoint = (signal_bar.high + signal_bar.low) / 2
```

- **Long position**: stop at `midpoint - sl_midpoint_offset`
- **Short position**: stop at `midpoint + sl_midpoint_offset`

**Example** (DAX signal bar: high = 16,050, low = 15,980, offset = 5):

```
Midpoint = (16,050 + 15,980) / 2 = 16,015
Long stop  = 16,015 - 5 = 16,010
Short stop = 16,015 + 5 = 16,020
```

---

## Exit Strategies

Four parameterized exit modes cover the range of exit approaches. These are in addition to the stop loss, which always remains active.

### End of Day (`EndOfDay`)

All open positions are closed at the configured end-of-day time (`exit_eod_time`, default: 17:30 exchange local time). The position is closed at the candle's close price.

### Trailing Stop (`TrailingStop`)

A trailing stop follows favorable price movement at a fixed distance.

- **Long**: trail level = `best_high - trailing_stop_distance`
- **Short**: trail level = `best_low + trailing_stop_distance`

The trailing stop only activates after unrealized profit exceeds `trailing_stop_activation` (default: 0, meaning immediate activation). Once active, if price retraces to the trail level, the position is closed.

### Fixed Take Profit (`FixedTakeProfit`)

The position is closed when unrealized profit reaches `fixed_tp_points` (default: 100 points).

- **Long**: take profit at `entry + fixed_tp_points`
- **Short**: take profit at `entry - fixed_tp_points`

### Close at Time (`CloseAtTime`)

All open positions are closed at a specific clock time (`close_at_time`, default: 15:00 exchange local time). Similar to end-of-day but allows an earlier exit.

### No Automatic Exit (`None`)

Positions run until they are stopped out (by the initial stop loss or trailing stop). No time-based or profit-based exit is applied.

---

## Adding to Winners

When a trade moves favorably, the strategy can add additional positions at configured intervals. This mechanism is disabled by default (`add_to_winners_enabled = false`).

### How It Works

1. **Trigger**: An add triggers when price moves `add_every_points * N` from the original entry in the favorable direction, where N is the add number (1st, 2nd, 3rd...).
   - 1st add: entry + 50 points (for a long with `add_every_points = 50`)
   - 2nd add: entry + 100 points
   - 3rd add: entry + 150 points

2. **Size**: Each add has size `position_size * add_size_multiplier`. A multiplier of 1.0 means the same size as the initial position; 2.0 means double.

3. **Maximum**: The `max_additions` parameter (default: 3) caps the number of add-on positions per trade.

4. **Stop tightening**: When `move_sl_on_add` is true (default), the stop loss is tightened on each add. The new stop is set to the previous add's entry price (or the original entry for the first add), offset by `add_sl_offset` in the adverse direction.

### Worked Example

Long entry at 16,000, `add_every_points = 50`, `add_sl_offset = 5`, `max_additions = 3`:

| Event | Price | New Stop Loss | Rationale |
|---|---|---|---|
| Entry | 16,000 | 15,960 | Initial SL (fixed 40 points) |
| 1st add | 16,050 | 15,995 | SL moves to entry(16,000) - offset(5) |
| 2nd add | 16,100 | 16,045 | SL moves to 1st add(16,050) - offset(5) |
| 3rd add | 16,150 | 16,095 | SL moves to 2nd add(16,100) - offset(5) |
| Max reached | -- | 16,095 | No further adds |

If the market reverses and hits 16,095, all positions (base + 3 adds) close at that level.

---

## Position Update Processing Order

When processing each candle against an open position, the engine checks exit conditions in a strict priority order. This order is critical for correctness, especially when a single candle could satisfy multiple conditions.

The 6-step priority (implemented in `Position::update`):

1. **Stop loss check** -- Was the stop hit? (candle low <= SL for longs, candle high >= SL for shorts)
2. **Update best price** -- Track the highest high (longs) or lowest low (shorts) for trailing stop
3. **Trailing stop check** -- Has the trailing stop been hit? (only if `exit_mode = TrailingStop`)
4. **Take profit check** -- Has the TP level been reached? (only if `exit_mode = FixedTakeProfit`)
5. **Adding to winners** -- Should an add-on position be placed? (only if `add_to_winners_enabled`)
6. **Time-based exit** -- Has the EOD or close-at-time been reached?

If a condition at step N closes the position, steps N+1 through 6 are skipped.

---

## Conservative Assumption

When a single candle contains both the stop loss level and a favorable level (take profit, trailing stop, or add trigger), the engine assumes the stop loss was hit first. This is the "conservative assumption" -- it prevents the backtest from crediting favorable outcomes that may not have occurred in reality.

For example, if a long position has SL at 15,960 and TP at 16,100, and a candle prints high = 16,110, low = 15,950:

- Both SL and TP were possible on this candle
- The engine records a **stop loss exit at 15,960** (conservative)
- The take profit is never checked because the SL check (step 1) fires first

This design prevents overly optimistic backtest results.

---

## Full Parameter Reference

### Signal Detection

| Parameter | Type | Default | Description |
|---|---|---|---|
| `instrument` | `Instrument` | `Dax` | Target index: Dax, Ftse, Nasdaq, Dow |
| `signal_bar_index` | `u8` | `2` | Which 15-min candle after open (1-based) |
| `candle_interval_minutes` | `u16` | `15` | Candle timeframe in minutes |
| `entry_offset_points` | `Decimal` | `2` | Points above/below signal bar for entry |
| `allow_both_sides` | `bool` | `true` | Both buy and sell can trigger same session |

### Stop Loss

| Parameter | Type | Default | Description |
|---|---|---|---|
| `sl_mode` | `StopLossMode` | `FixedPoints` | SignalBarExtreme, FixedPoints, or Midpoint |
| `sl_fixed_points` | `Decimal` | `40` | Fixed SL distance (FixedPoints mode) |
| `sl_midpoint_offset` | `Decimal` | `5` | Buffer beyond midpoint (Midpoint mode) |
| `sl_scale_with_index` | `bool` | `false` | Scale SL with index level |
| `sl_scale_baseline` | `Decimal` | `12000` | Baseline index level for scaling |

### Exit Strategy

| Parameter | Type | Default | Description |
|---|---|---|---|
| `exit_mode` | `ExitMode` | `EndOfDay` | EndOfDay, TrailingStop, FixedTakeProfit, CloseAtTime, None |
| `exit_eod_time` | `NaiveTime` | `17:30` | EOD flatten time (exchange local) |
| `trailing_stop_distance` | `Decimal` | `30` | Trail distance in points |
| `trailing_stop_activation` | `Decimal` | `0` | Min profit before trailing activates |
| `fixed_tp_points` | `Decimal` | `100` | Take profit distance in points |
| `close_at_time` | `NaiveTime` | `15:00` | Close-at-time exit (exchange local) |

### Adding to Winners

| Parameter | Type | Default | Description |
|---|---|---|---|
| `add_to_winners_enabled` | `bool` | `false` | Enable adding to winners |
| `add_every_points` | `Decimal` | `50` | Add every X points of favorable move |
| `max_additions` | `u8` | `3` | Maximum adds per trade |
| `add_size_multiplier` | `Decimal` | `1` | Size of each add relative to initial |
| `move_sl_on_add` | `bool` | `true` | Tighten SL when adding |
| `add_sl_offset` | `Decimal` | `0` | Offset from previous add for new SL |

### Session Timing

| Parameter | Type | Default | Description |
|---|---|---|---|
| `session_open` | `Option<NaiveTime>` | per instrument | Override session open time |
| `session_close` | `Option<NaiveTime>` | per instrument | Override session close time |
| `signal_expiry_time` | `Option<NaiveTime>` | `None` | Time after which unfilled orders cancel |

### Backtest Scope

| Parameter | Type | Default | Description |
|---|---|---|---|
| `date_from` | `NaiveDate` | `2024-01-01` | Start date (inclusive) |
| `date_to` | `NaiveDate` | `2025-12-31` | End date (inclusive) |
| `initial_capital` | `Decimal` | `100,000` | Starting capital |
| `position_size` | `Decimal` | `1` | Base position size (lots/contracts) |
| `point_value` | `Decimal` | `1` | Cash value per point per lot |
| `commission_per_trade` | `Decimal` | `0` | Round-trip commission per trade |
| `slippage_points` | `Decimal` | `0.5` | Simulated slippage per fill |
| `exclude_dates` | `Vec<NaiveDate>` | `[]` | Dates to exclude (holidays, gaps) |

---

## Worked Example: Full Trade Lifecycle

**Setup**: DAX, 15 January 2024 (winter CET), default config (FixedPoints 40, EndOfDay 17:30, no adding).

**Step 1: Signal bar identified at 09:15 CET (08:15 UTC)**

```
Signal bar: O=16,000  H=16,050  L=15,980  C=16,030
Buy stop:   16,052 (16,050 + 2)
Sell stop:  15,978 (15,980 - 2)
```

**Step 2: Buy stop triggers at 09:45 CET**

The 09:45 candle has high = 16,060. Since 16,060 >= 16,052, the buy stop fills at 16,052 (open was 16,040, which is below trigger, so fill at trigger).

```
Long position opened:
  Entry:     16,052
  Stop loss: 16,012 (16,052 - 40)
  Size:      1 lot
```

**Step 3: Position managed through the day**

Each subsequent 15-min candle is processed:
- 10:00 candle: H=16,080, L=16,045 -> SL not hit, best_price updates to 16,080
- 10:15 candle: H=16,090, L=16,060 -> SL not hit, best_price updates to 16,090
- ... (price stays above 16,012 all day)

**Step 4: End-of-day exit at 17:30 CET (16:30 UTC)**

The 16:30 UTC candle's local time is 17:30 CET, triggering the EOD exit.

```
Exit at candle close: 16,085
PnL = (16,085 - 16,052) * 1 = 33 points
Slippage deducted: 0.5 * 2 = 1.0 point (entry + exit)
Net PnL: 33 - 1.0 = 32.0 points
```

---

## PnL Computation

### Base Position

```
pnl_points = (exit_price - entry_price) * direction_multiplier
```

Where `direction_multiplier` is +1 for longs and -1 for shorts.

### With Add-on Positions

Each add-on position has its own PnL:

```
add_pnl = (exit_price - add_entry_price) * direction_multiplier * add_size
```

Total PnL combines the base and all adds, then deducts costs:

```
total_fills = 1 + number_of_adds
total_commission = commission_per_trade * total_fills
total_slippage = slippage_points * total_fills * 2  (entry + exit per fill)
pnl_with_adds = base_pnl * position_size + sum(add_pnl) - total_commission - total_slippage
```

---

## Source Code Reference

All strategy logic resides in `engine/src/strategy/`:

| File | Purpose |
|---|---|
| `mod.rs` | Module declarations and public re-exports |
| `config.rs` | `StrategyConfig` struct with all parameters |
| `types.rs` | `Direction`, `StopLossMode`, `ExitMode`, `PositionStatus` enums |
| `signal.rs` | `SignalBar` struct and `find_signal_bar` function |
| `order.rs` | `PendingOrder` and `generate_orders` with SL computation |
| `fill.rs` | `check_fill` and `determine_fill_order` for fill simulation |
| `position.rs` | `Position` struct with `update` (6-step priority) and `close` |
| `add_to_winners.rs` | `check_add_trigger` for adding to winning positions |
| `trade.rs` | `Trade` and `AddResult` with PnL computation |
