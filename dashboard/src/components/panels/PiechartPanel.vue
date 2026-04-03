<template>
  <v-chart :option="chartOption" autoresize class="w-full h-full" />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { PieChart } from 'echarts/charts'
import { TooltipComponent, LegendComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import type { PrometheusResponse, PrometheusMetric } from '@/types'

use([PieChart, TooltipComponent, LegendComponent, CanvasRenderer])

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()

const chartOption = computed(() => {
  const result = (props.data as PrometheusResponse)?.data?.result || []
  const pieData = result.map((r: PrometheusMetric) => ({
    name: Object.values(r.metric).join(' ') || 'unknown',
    value: parseFloat(r.value?.[1] || '0'),
  }))

  return {
    tooltip: { trigger: 'item' },
    legend: { bottom: 0 },
    series: [
      {
        type: 'pie',
        radius: ['40%', '70%'],
        data: pieData,
      },
    ],
  }
})
</script>
