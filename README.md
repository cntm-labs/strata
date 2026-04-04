<div align="center">

# strata

**Open-source observability dashboard — a general-purpose Grafana alternative with full UI/UX control.**

[![CI](https://github.com/cntm-labs/strata/actions/workflows/ci.yml/badge.svg)](https://github.com/cntm-labs/strata/actions/workflows/ci.yml)
[![Security](https://github.com/cntm-labs/strata/actions/workflows/security.yml/badge.svg)](https://github.com/cntm-labs/strata/actions/workflows/security.yml)
[![Release](https://github.com/cntm-labs/strata/actions/workflows/release.yml/badge.svg)](https://github.com/cntm-labs/strata/actions/workflows/release.yml)
[![codecov](https://codecov.io/gh/cntm-labs/strata/branch/main/graph/badge.svg)](https://codecov.io/gh/cntm-labs/strata)

[![Rust](https://img.shields.io/badge/Rust-3.5k_LOC-dea584?logo=rust&logoColor=white)](resource/)
[![TypeScript](https://img.shields.io/badge/TypeScript-3.5k_LOC-3178c6?logo=typescript&logoColor=white)](dashboard/src/)
[![Total Lines](https://img.shields.io/badge/Total-7k+_LOC-blue)](./)

[![Rust](https://img.shields.io/badge/Rust-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/Axum-dea584?logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)
[![Vue 3](https://img.shields.io/badge/Vue_3-4FC08D?logo=vuedotjs&logoColor=white)](https://vuejs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Vite](https://img.shields.io/badge/Vite-646CFF?logo=vite&logoColor=white)](https://vite.dev/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-06B6D4?logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![DaisyUI](https://img.shields.io/badge/DaisyUI-5A0EF8?logo=daisyui&logoColor=white)](https://daisyui.com/)
[![ECharts](https://img.shields.io/badge/ECharts-AA344D?logo=apacheecharts&logoColor=white)](https://echarts.apache.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-4169E1?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Prometheus](https://img.shields.io/badge/Prometheus-E6522C?logo=prometheus&logoColor=white)](https://prometheus.io/)
[![Loki](https://img.shields.io/badge/Loki-F46800?logo=grafana&logoColor=white)](https://grafana.com/oss/loki/)
[![Docker](https://img.shields.io/badge/Docker-2496ED?logo=docker&logoColor=white)](https://www.docker.com/)

</div>

---

Rust backend (Axum) + Vue 3 frontend. Supports Prometheus (PromQL), Loki (LogQL), and PostgreSQL (SQL) data sources.

## Features

- **Dashboard builder** — drag-and-drop panels with 12-column grid layout
- **8 panel types** — timeseries (uPlot), stat, gauge, bar, heatmap, piechart (ECharts), table (AG Grid), logs (xterm.js)
- **Explore mode** — ad-hoc queries with auto-detection of result type
- **Alerting** — rule-based alerts with SMS/Email delivery via Chorus
- **Templates** — built-in dashboard templates for Node Exporter, PostgreSQL, Redis, and more
- **Data source proxy** — backend proxies all queries, never exposes Prometheus/Loki directly to the browser

## Quick Start

```bash
docker compose up -d
```

Open [http://localhost:3000](http://localhost:3000)

## Development

```bash
# Backend
cd resource && cargo run

# Frontend
cd dashboard && bun install && bun run dev
```

## License

[MIT](LICENSE)
