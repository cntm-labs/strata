<template>
  <v-chart :option="chartOption" autoresize class="w-full h-full" />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { HeatmapChart } from 'echarts/charts'
import { GridComponent, VisualMapComponent, TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import type { PrometheusResponse, PrometheusMetric } from '@/types'

use([HeatmapChart, GridComponent, VisualMapComponent, TooltipComponent, CanvasRenderer])

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()

const chartOption = computed(() => {
  const result = (props.data as PrometheusResponse)?.data?.result || []
  const heatmapData: [number, number, number][] = []

  result.forEach((r: PrometheusMetric, seriesIdx: number) => {
    const values = r.values || []
    values.forEach((v: [number, string], timeIdx: number) => {
      heatmapData.push([timeIdx, seriesIdx, parseFloat(v[1])])
    })
  })

  return {
    tooltip: {},
    xAxis: { type: 'category' },
    yAxis: {
      type: 'category',
      data: result.map((_: PrometheusMetric, i: number) => `Series ${i + 1}`),
    },
    visualMap: { min: 0, max: 100, calculable: true },
    series: [{ type: 'heatmap', data: heatmapData }],
  }
})
</script>
