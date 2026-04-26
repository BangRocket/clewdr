# Responsive Audit -- feature/cookie-cost-tracking

Captured 2026-04-26 against branch top commit `735dd8e`.

Viewports tested: 375 / 414 / 768 / 1024 / 1280 / 1920.

## Methodology

- Playwright (`@playwright/test` 1.59.1) + bundled Chromium, full-page
  screenshots at each viewport.
- For each viewport: login screen, post-login, then each top-level tab
  (claude / usage / config / token), and for the usage tab also each
  sub-tab (overview / by_cookie / graveyard).
- Capture script: `frontend/scripts/capture-screenshots.ts`. Run with
  the backend serving the built bundle on port 8484:

  ```bash
  ADMIN_TOKEN=... FRONTEND_URL=http://127.0.0.1:8484 \
    node --experimental-strip-types frontend/scripts/capture-screenshots.ts
  ```

  Output written to `screenshots/` (gitignored).
- Backend ran with the existing `clewdr.toml` (admin password sourced
  directly from that config, not the runtime log). Cookie list was
  empty; this is a clean-slate audit. Items that depend on populated
  state (long cookie strings, many graveyard cards, real chart data)
  are flagged with `[empty-state]` so Phase 15 knows to re-verify them
  with seeded data.
- 54 PNGs captured -- 9 per viewport (1 login, 1 after-login,
  4 top-tabs, 3 usage sub-tabs). Sparklines, populated charts, and
  cookie-list overflow could not be observed at this snapshot; the
  static-analysis notes below cover those.

## Findings by viewport

### iphone-se (375 x 812)

#### Header (all tabs)

- [ ] **layout**: The version `<h2>` is rendered as a single mono
      string `v0.12.24 by Xerxes-2 <dspxue@gmail.com>:LeenHawk <leenhawk@leenhawk.com> | profile: debug | mode: prod | no_fs: false`
      and wraps across 4 lines. The two email addresses + the
      `:LeenHawk` separator collide visually so it reads as
      `<dspxue@gmail.com>:LeenHawk` on one line. Source:
      `frontend/src/components/layout/Header.tsx:20`.
      Fix candidate: render version on the first line and authors / build
      flags on subsequent lines, or hide the build-flag suffix below
      `sm` and reveal it as a tooltip.
- [ ] **layout**: `mb-10` (40px) on the header creates significant
      blank space between version-string and the card on a viewport
      that already has to scroll. Tighten to `mb-6` on small.

#### claude tab

- [ ] **navigation**: 4 top tabs (Claude / Usage / Config / Auth) plus
      2 sub-tabs (Submit Cookie / Cookie Status) consume ~140px of
      vertical space before any content renders. Acceptable but worth
      noting; consider compacting the sub-tab row into a segmented
      control on `sm` only.
- [ ] **density [populated state, static-analysis]**: When cookies
      exist, `CookieVisualization` renders 3 stacked `CookieSection`
      blocks (valid / exhausted / invalid). Each row has flex layout
      with a value pill + a flex-grow value; on 375px the cookie
      string truncation is `min-w-0 mb-1 sm:mb-0` (lines 639/687/734
      of `CookieVisualization.tsx`) so it should wrap, but with
      multiple action buttons stacking it can exceed the card's
      ~340px content width. Flagged for re-audit when seeded.

#### usage tab -- overview

- [ ] **density**: 4 summary cards stack vertically (`grid-cols-1`)
      consuming ~620px before the sub-tab navigation. With the header
      that pushes the chart area below the fold by ~1100px on a
      812px-tall viewport. Fix candidate: 2 x 2 grid on `xs`+
      (`grid-cols-2 sm:grid-cols-2 lg:grid-cols-4`), and shrink card
      vertical padding from `p-3` to `p-2` below `sm`.
- [ ] **layout [empty-state]**: Charts render a 240px-tall placeholder
      with centered "No data" text. Two of these stacked = ~500px of
      empty dark space on the smallest viewport. Fix candidate:
      collapse empty charts to a single line ("No cost data yet"),
      with the full chart only rendering when `data.length > 0`.
      Source: `frontend/src/components/usage/charts/CostLineChart.tsx:31`,
      `TokenBarChart.tsx` (same pattern).

#### usage tab -- by cookie

- [ ] **density [partly empty-state]**: Sparklines are intentionally
      hidden below `sm` (`hidden h-12 w-32 sm:block`,
      `UsagePerCookie.tsx:90`). On 375px users only see the
      `cookie_ellipse` + state badge + a one-line cost-and-snapshot
      summary. This is OK but means the "By cookie" tab provides
      little visual signal on phones; consider promoting the cost to
      the right side of each row so users get a glanceable column.
- [ ] **touch [populated state]**: Each row is `min-h-[64px]` -- OK.
      But `c.cookie_ellipse` is rendered in `font-mono text-xs` which
      is ~10px and inside `truncate` -- legible but small for a click
      target's primary identifier. Consider bumping to `text-sm` on
      mobile.

#### usage tab -- graveyard

- [ ] **layout [empty-state]**: Same as by-cookie -- only a "No dead
      cookies" string visible; nothing to validate density-wise.
      Re-audit after Phase 15 with seeded dead cookies.
- [ ] **density [populated state, static-analysis]**: Cards in
      `grid-cols-1` on small (`Graveyard.tsx:36`). With reason text
      and final-snapshot line, expected ~120-140px tall each. Three
      dead cookies = ~420px; that is fine, but >5 will require a "show
      more" or virtualization. Defer.

#### config tab

- [ ] **layout**: Top-of-card flex row (`<h3>Configuration</h3>` +
      `Save Configuration` button, `flex justify-between items-center`,
      `index.tsx:176`) crams the button against the title because
      "Save Configuration" is wider than the title and there is no
      `flex-wrap` or breakpoint. Save button visually dominates the
      title at 375px. Fix: stack vertically on small
      (`flex-col sm:flex-row`), or use a sticky bottom-action bar.
- [ ] **layout**: Checkbox group inside API Settings uses
      `grid grid-cols-2 gap-x-6 gap-y-3` (`ConfigForm.tsx:121`) which
      is 2 columns at every breakpoint. On 375px each cell is ~150px
      wide and labels like "Sanitize messages (trim whitespace)" and
      "Claude Code telemetry emulation" wrap to 3 lines, making the
      grid uneven and hard to scan. Fix:
      `grid-cols-1 sm:grid-cols-2`.
- [ ] **touch**: Native checkboxes are sized `w-4 h-4` (16x16,
      `ConfigCheckbox.tsx:23`). Below the 44x44 minimum. The
      surrounding `<label>` extends the hit-area horizontally, but
      vertical hit area is still ~16-20px. Fix: bump checkbox to
      `w-5 h-5` and add `min-h-[44px] py-2` on the label.
- [ ] **touch**: Server Settings `IP Address` / `Port` fields and
      `App Settings` `Check for updates` / `Auto update` checkboxes
      live in a 2-column grid (`grid-cols-1 md:grid-cols-2 gap-4`,
      `ConfigForm.tsx:28` and `:111`). At 375px (below `md`=768) it
      collapses to 1 column -- OK. Note: the App Settings "Auto
      update" label (one word per line) on iphone-se is squished.
      Likely cosmetic.
- [ ] **layout**: The Save Configuration button uses `py-2 px-4`
      gradient styling. Touch target ~36-40px tall. Below 44x44.
      Bump.

#### token tab

- [ ] **layout**: "Auth Status" + bright red "Logout" button in a flex
      row (`LogoutPanel`) -- looks fine on iphone-se (image confirms).
      No issue.

### iphone-pro-max (414 x 896)

#### Same patterns as iphone-se

- [ ] **layout**: Same Header version-wrap problem (4 lines).
- [ ] **density**: Same vertical-stack of 4 summary cards on usage
      overview (still `grid-cols-1` -- the `sm:` breakpoint at 640px
      hasn't kicked in yet). 414px is the most common modern phone
      width and the current layout wastes vertical space.
- [ ] **layout**: Same Config Save Configuration button cramming.
- [ ] **layout**: Config checkbox grid still 2-column with multi-line
      wrap. Visible in `iphone-pro-max-config.png`: "Preserve Chats",
      "Web Search", "Enable web count_tokens", "Sanitize messages
      (trim whitespace)", "Claude Code telemetry emulation" form a
      ragged 3-row x 2-col grid with uneven cell heights.

### ipad-portrait (768 x 1024)

- [ ] **density**: At 768px, the `sm:grid-cols-2` breakpoint kicks in
      so summary cards become 2 x 2 -- much better. But the chart
      empty-state is still 240px tall x 2 = 500px of dead space below
      "No data". Same issue as iphone-se but proportionally larger.
- [ ] **layout**: At 768px the App container is `max-w-xl`
      (`App.tsx:70`) for non-usage tabs which is ~576px. That leaves
      ~190px of gutter on each side -- looks OK but slightly cramped
      for long form fields like Claude Web Custom Prompt. The
      `max-w-full lg:max-w-5xl xl:max-w-6xl` rule for the usage tab
      means at 768px the usage card fills the whole viewport, which
      is correct.
- [ ] **layout**: `Header.tsx` version line wraps to 2 lines at 768px:
      `... <leenhawk@leenhawk.com> | profile:` / `debug | mode: prod`.
      Acceptable but ugly. See iphone-se finding.

### ipad-landscape (1024 x 768)

- [ ] **density**: Summary cards still 2 x 2 (`sm:grid-cols-2`); the
      `lg:grid-cols-4` breakpoint at 1024px should produce 4 x 1
      but the screenshot shows 2 x 2. Verify whether the App's
      `lg:max-w-5xl` constraint is interfering at the boundary. If
      cards measure ~512px wide (5xl/2) the layout is fine; if they
      look constrained, lower the breakpoint or drop the max-width.
- [ ] **layout**: Same chart empty-space issue, now stretched
      horizontally. No content but ~500px vertical waste.
- [ ] **layout**: For non-usage tabs at 1024px the card is
      `md:max-w-xl` (~576px) centered in 1024px viewport -- 224px of
      gutter each side. Acceptable design choice; flag for review:
      should Config/Claude widen on `lg`+? Currently only Usage does.

### laptop (1280 x 800)

- [ ] **density**: Summary cards confirmed 4 x 1 (Lifetime cost / Input
      tokens / Output tokens / Active cookies). Looks good.
- [ ] **layout**: Sub-tab row (Overview / By cookie / Graveyard) is
      full-width across the usage card; tab indicator underline aligns
      with content well. OK.
- [ ] **layout**: Chart empty-state still wastes ~460px vertically
      (240 + 220). Fix proposed above.
- [ ] **layout**: Non-usage tabs (claude, config, token) keep the
      `md:max-w-xl` (~576px) constraint, so on a 1280px-wide screen
      we get a narrow card centered with ~350px of gutter each side.
      That is the existing design; flag for review whether usage's
      `lg:max-w-5xl` rule should be applied to other tabs too. Right
      now only the Usage card uses the wider variant.

### desktop (1920 x 1080)

- [ ] **layout**: Usage tab card is constrained to `xl:max-w-6xl`
      (~1152px). Reasonable -- 768px gutter total -- but the empty
      chart placeholders consume the full width, so on 1920px
      viewport you get ~1100px-wide empty rectangles labeled "No
      data". Visually noisy.
- [ ] **layout**: Summary cards at 1920px are 4 x 1 stretched -- each
      ~280px wide containing one number and a label. They look
      balanced but the value `0` (test data) feels lonely with the
      surrounding whitespace. Cosmetic only.
- [ ] **layout**: Non-usage tabs are still at `md:max-w-xl` meaning a
      ~576px card centered in 1920px (672px gutter each side). The
      design is intentional but feels notably empty on desktop. See
      laptop note.

## Summary by category

### Layout (block: overflow / broken alignment / dead space)

- iphone-se / iphone-pro-max -- header: version string wraps 4 lines,
  emails collide visually with `:LeenHawk` separator.
  (`layout/Header.tsx:20`)
- iphone-se / iphone-pro-max -- config: Save Configuration button
  crams the Configuration heading; needs `flex-col sm:flex-row` or a
  bottom action bar. (`config/index.tsx:176-187`)
- iphone-se / iphone-pro-max -- config: Checkbox grid is
  `grid-cols-2` at every breakpoint, producing 3-line label wraps and
  a ragged 3 x 2 grid. (`config/ConfigForm.tsx:121`)
- All viewports -- usage overview: Empty-state charts render as
  240px-tall centered "No data" boxes; two stacked = ~500px of dead
  space. (`charts/CostLineChart.tsx:31`, `charts/TokenBarChart.tsx`)
- ipad-portrait / ipad-landscape -- header: version line wraps to 2
  lines and looks ugly even at moderate widths.
- desktop / laptop -- non-usage tabs constrained to `md:max-w-xl`;
  Usage's `lg:max-w-5xl xl:max-w-6xl` not applied elsewhere.
  (`App.tsx:66-72`)

### Density (block: too cramped or info hidden)

- iphone-se / iphone-pro-max -- usage overview: 4 summary cards stack
  vertically (~620px) before any chart appears.
  (`UsageDashboard.tsx:51`)
- iphone-se / iphone-pro-max -- by-cookie: Sparklines hidden by
  design below `sm`. Compensate with a glanceable cost column.
  (`UsagePerCookie.tsx:90`)
- iphone-se -- claude tab: Top + sub-tab navigation = ~140px before
  any content; consider compacting on smallest viewport.

### Touch targets (block: <44 x 44)

- All viewports -- config: Native checkboxes `w-4 h-4` = 16 x 16.
  Wrapping label adds horizontal hit area but vertical is still
  small. (`config/ConfigCheckbox.tsx:23`)
- All viewports -- config: Save Configuration button `py-2 px-4` is
  ~36-40px tall. Below 44. (`config/index.tsx:182`)
- iphone-se / iphone-pro-max -- by-cookie: Each row is `min-h-[64px]`
  which is fine, but the cookie ellipse identifier is `text-xs`
  (10-11px) -- legibility concern at touch distance.
  (`UsagePerCookie.tsx:65`)

### Navigation (block: tabs/drawer awkward at width)

- iphone-se / iphone-pro-max -- usage detail drawer: Drawer is
  `w-full max-w-full md:max-w-[640px]`; on phones it occupies the
  full width and is dismissable by clicking the backdrop or the
  close button. Close button is `min-h-[44px] min-w-[44px]` -- OK.
  (`CookieUsageDetail.tsx:96`)
- iphone-se -- usage detail drawer: Filter pill rows
  (bucket: hour/day, source: all/web/code) use `flex flex-wrap` and
  buttons are `min-h-[36px]` -- below 44.
  (`CookieUsageDetail.tsx:107-138`)
- All viewports -- top tab navigation: 4 tabs share one row with
  i18n labels. On iphone-se the labels (Claude / Usage / Config /
  Auth) fit because they're short, but a longer language could
  overflow horizontally. No `overflow-x-auto` fallback.
  (Verify with `zh` locale in Phase 15.)

### Empty states / static-analysis re-audit needed

- Cookie list (claude tab) populated state -- need seeded cookies.
- Per-cookie sparklines (`UsagePerCookie`) -- sparklines invisible
  in this audit.
- Cost & token charts (`UsageOverview`) -- need real time-series.
- Graveyard cards with real reasons -- only empty state observed.
- `zh` locale -- only `en` was captured.

## Acceptance criteria for Phase 15

- All Layout / Touch issues above resolved or explicitly deferred
  with rationale.
- Density issues addressed via collapsible/responsive disclosure
  where possible (e.g. summary cards 2 x 2 from 375px upward; charts
  collapse when empty).
- No horizontal scrollbars at any viewport (except inside
  intentionally-scrollable elements like the per-cookie row's
  optional sparkline at `sm`+).
- All interactive elements meet 44 x 44 px touch target on touch
  devices.
- Re-run `frontend/scripts/capture-screenshots.ts` after fixes and
  visually diff key screens (iphone-se overview/config, ipad-portrait
  overview, desktop usage).
- Re-audit with seeded data (>=1 valid cookie, >=1 dead cookie, real
  time-series buckets) and `zh` locale to verify items flagged
  `[empty-state]` and i18n overflow.

## Phase 15 fixes applied 2026-04-26

Re-capture pass: 2026-04-26 -- 54 PNGs in `screenshots/`. Spot-checked
iphone-se config / iphone-se overview / ipad-portrait overview /
desktop overview / iphone-pro-max config and confirmed each fix
landed.

### Touch targets (commit 84c2913)
- [x] `ConfigCheckbox.tsx`: wrapping label now `min-h-[44px] py-2`,
      visual checkbox bumped from `w-4 h-4` to `w-5 h-5` and
      `flex-shrink-0` to keep size when the label wraps.
- [x] `config/index.tsx:182`: Save Configuration button now has
      `min-h-[44px]` (overrides the `py-2` shrink).
- [x] `CookieUsageDetail.tsx:114,130`: bucket and source filter pills
      bumped from `min-h-[36px]` to `min-h-[44px]`.

### Layout (commit 022e575)
- [x] `UsageDashboard.tsx:51`: summary cards switched from
      `grid-cols-1 sm:grid-cols-2 lg:grid-cols-4` to
      `grid-cols-2 lg:grid-cols-4`. Phones get 2x2 (~310px tall)
      instead of 1x4 (~620px tall); ipad-portrait already had 2x2
      and is unchanged; laptop / desktop get 4x1 at the existing
      `lg` boundary.
- [x] `config/index.tsx:176-187`: Configuration heading + Save
      button now `flex-col gap-3 sm:flex-row sm:items-center
      sm:justify-between` and the button is `w-full sm:w-auto`. No
      more cramming on phones; desktop layout unchanged.
- [x] `ConfigForm.tsx:121`: API Settings checkbox grid switched
      from `grid-cols-2` (always) to `grid-cols-1 sm:grid-cols-2`.
      Phones get a single readable column; tablets+/desktops keep
      two columns.
- [x] `Header.tsx`: split the multi-line `VERSION_INFO` on `\n` and
      hide the metadata lines (`profile:`, `mode:`, `no_fs:`) below
      `md`. Header `mb-10` reduced to `mb-6 sm:mb-10`. Version line
      uses `text-xs sm:text-sm break-words` so the long author/email
      line wraps on phones without spilling. Phones now show 2-3
      lines of header instead of 4-5.

### Empty state (commit a9ae0af)
- [x] `CostLineChart.tsx`, `TokenBarChart.tsx`: added
      `emptyHeight` prop (default 64). Empty placeholders now ~64px
      tall instead of 240/220px. Verified: iphone-se overview
      went from ~1100px tall total to ~700px tall; desktop overview
      went from ~500px of empty rectangles to ~130px.

### Outstanding (deferred to Phase 16)

- [empty-state] Re-audit with seeded valid + dead cookies (no real
  data was available at audit time, so populated densities for
  `CookieVisualization`, `UsagePerCookie` sparklines, `Graveyard`
  cards, and the cookie detail drawer charts cannot be confirmed
  yet).
- [zh-locale] Re-capture in `zh` locale; the audit script only
  exercised the default `en` locale, and the top tab labels have
  no `overflow-x-auto` fallback if they collide.
- [layout, deferred] Non-usage tabs (`claude`, `config`, `token`)
  still constrained to `md:max-w-xl` (~576px) on `lg`+. The audit
  flagged this as "feels notably empty on desktop" but it's an
  intentional design decision; revisit if the Config form gains
  wider content (e.g. side-by-side panels).
- [touch, deferred] CookieVisualization has small context-toggle
  buttons (`px-3 py-1.5 text-xs`) and a small retry button on
  config error states (`py-1 px-3`). These were flagged
  `[populated state, static-analysis]` in the audit and only
  surface in narrow paths; revisit during the seeded-state re-audit.
