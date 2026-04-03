<template>
  <div class="flex gap-4 h-[calc(100vh-6rem)]">
    <!-- Main area -->
    <div class="flex-1 flex flex-col min-w-0">
      <!-- Controls -->
      <div class="flex gap-3 items-end mb-4">
        <div class="form-control flex-1">
          <label class="label text-xs">Datasource</label>
          <Select
            v-model="datasourceId"
            :options="datasources"
            optionLabel="name"
            optionValue="id"
            placeholder="Select datasource"
            class="w-full"
          />
        </div>
        <div class="form-control w-28">
          <label class="label text-xs">Range</label>
          <Select
            v-model="timeRange"
            :options="timeRangeOptions"
            optionLabel="label"
            optionValue="value"
          />
        </div>
        <button class="btn btn-primary" :disabled="!datasourceId || !query" @click="runQuery">
          <i class="pi pi-play mr-1" /> Run
        </button>
      </div>

      <!-- Query editor -->
      <div class="mb-4">
        <MonacoEditor v-model="query" :language="queryLanguage" :height="100" />
      </div>

      <!-- Results -->
      <div class="flex-1 min-h-0 bg-base-200 rounded-lg overflow-auto">
        <div v-if="loading" class="flex justify-center p-8">
          <ProgressSpinner />
        </div>
        <div v-else-if="error" class="p-4 text-error">{{ error }}</div>
        <div v-else-if="result" class="h-full">
          <!-- Auto-detect and render -->
          <component :is="resultComponent" :data="result" :config="{}" class="h-full" />
        </div>
        <div v-else class="flex items-center justify-center h-full text-base-content/40">
          Run a query to see results
        </div>
      </div>
    </div>

    <!-- History sidebar -->
    <div class="w-72 bg-base-200 rounded-lg flex flex-col overflow-hidden shrink-0">
      <div class="p-3 font-semibold border-b border-base-300">History</div>
      <div class="flex-1 overflow-auto">
        <div
          v-for="item in history"
          :key="item.id"
          class="p-3 border-b border-base-300 hover:bg-base-300 cursor-pointer"
          @click="loadHistory(item)"
        >
          <div class="font-mono text-xs truncate">{{ item.query }}</div>
          <div class="text-xs text-base-content/40 mt-1">
            <span class="badge badge-xs mr-1">{{ item.query_type }}</span>
            {{ formatTime(item.created_at) }}
          </div>
        </div>
        <div v-if="history.length === 0" class="p-3 text-sm text-base-content/40">
          No history yet
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, type Component } from 'vue'
import { datasourcesApi } from '@/api/datasources'
import { exploreApi, type ExploreHistory } from '@/api/explore'
import MonacoEditor from '@/components/MonacoEditor.vue'
import TimeseriesPanel from '@/components/panels/TimeseriesPanel.vue'
import StatPanel from '@/components/panels/StatPanel.vue'
import TablePanel from '@/components/panels/TablePanel.vue'
import LogsPanel from '@/components/panels/LogsPanel.vue'
import Select from 'primevue/select'
import ProgressSpinner from 'primevue/progressspinner'
import type { Datasource, PrometheusResponse } from '@/types'

const datasources = ref<Datasource[]>([])
const datasourceId = ref('')
const query = ref('')
const timeRange = ref('1h')
const result = ref<unknown>(null)
const loading = ref(false)
const error = ref('')
const history = ref<ExploreHistory[]>([])

const timeRangeOptions = [
  { label: '5m', value: '5m' },
  { label: '15m', value: '15m' },
  { label: '1h', value: '1h' },
  { label: '3h', value: '3h' },
  { label: '6h', value: '6h' },
  { label: '24h', value: '24h' },
]

const rangeMap: Record<string, number> = {
  '5m': 300,
  '15m': 900,
  '1h': 3600,
  '3h': 10800,
  '6h': 21600,
  '24h': 86400,
}

const selectedDs = computed(() => datasources.value.find((d) => d.id === datasourceId.value))

const queryLanguage = computed(() => {
  if (selectedDs.value?.type === 'postgresql') return 'sql'
  return 'promql'
})

const resultComponent = computed<Component>(() => {
  if (!result.value) return StatPanel
  const data = result.value as PrometheusResponse
  const resultType = data?.data?.resultType

  // Prometheus result types
  if (resultType === 'matrix') return TimeseriesPanel
  if (resultType === 'vector') return TablePanel
  if (resultType === 'scalar' || resultType === 'string') return StatPanel

  // Loki streams
  if (data?.data?.result?.[0]?.values && selectedDs.value?.type === 'loki') return LogsPanel

  // PostgreSQL / array results
  if (Array.isArray(data)) return TablePanel

  return StatPanel
})

async function runQuery() {
  loading.value = true
  error.value = ''
  try {
    const now = Math.floor(Date.now() / 1000)
    const duration = rangeMap[timeRange.value] || 3600
    result.value = await exploreApi.query({
      datasource_id: datasourceId.value,
      query: query.value,
      start: (now - duration).toString(),
      end: now.toString(),
      step: Math.max(Math.floor(duration / 250), 15).toString(),
    })
    await loadHistory_()
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : 'Query failed'
  } finally {
    loading.value = false
  }
}

function loadHistory(item: ExploreHistory) {
  datasourceId.value = item.datasource_id
  query.value = item.query
}

function formatTime(iso: string) {
  return new Date(iso).toLocaleTimeString()
}

async function loadHistory_() {
  history.value = await exploreApi.history({ limit: 30 })
}

onMounted(async () => {
  datasources.value = await datasourcesApi.list()
  await loadHistory_()
})
</script>
