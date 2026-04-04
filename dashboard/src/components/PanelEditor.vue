<template>
  <Dialog v-model:visible="visible" header="Edit Panel" :style="{ width: '640px' }" modal>
    <form @submit.prevent="save" class="space-y-4">
      <div class="form-control">
        <label class="label">Title</label>
        <InputText v-model="form.title" class="w-full" required />
      </div>

      <div class="form-control">
        <label class="label">Type</label>
        <Select
          v-model="form.type"
          :options="panelTypes"
          optionLabel="label"
          optionValue="value"
          class="w-full"
        />
      </div>

      <div class="form-control">
        <label class="label">Datasource</label>
        <Select
          v-model="form.datasource_id"
          :options="datasources"
          optionLabel="name"
          optionValue="id"
          class="w-full"
          placeholder="Select datasource"
        />
      </div>

      <div class="form-control">
        <label class="label">Query</label>
        <MonacoEditor v-model="form.query" :language="queryLanguage" :height="120" />
      </div>

      <div class="grid grid-cols-4 gap-2">
        <div class="form-control">
          <label class="label text-xs">X</label>
          <InputNumber v-model="form.x" :min="0" :max="11" />
        </div>
        <div class="form-control">
          <label class="label text-xs">Y</label>
          <InputNumber v-model="form.y" :min="0" />
        </div>
        <div class="form-control">
          <label class="label text-xs">Width</label>
          <InputNumber v-model="form.w" :min="1" :max="12" />
        </div>
        <div class="form-control">
          <label class="label text-xs">Height</label>
          <InputNumber v-model="form.h" :min="1" />
        </div>
      </div>

      <div class="flex gap-2 justify-end">
        <button type="button" class="btn btn-ghost" @click="visible = false">Cancel</button>
        <button type="submit" class="btn btn-primary">Save</button>
      </div>
    </form>
  </Dialog>
</template>

<script setup lang="ts">
import { reactive, ref, computed, onMounted, watch } from 'vue'
import { datasourcesApi } from '@/api/datasources'
import MonacoEditor from './MonacoEditor.vue'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import InputNumber from 'primevue/inputnumber'
import Select from 'primevue/select'
import type { Datasource, Panel } from '@/types'

const props = defineProps<{ panel?: Panel }>()
const emit = defineEmits<{
  save: [
    data: {
      title: string
      type: string
      datasource_id: string
      query: string
      position: { x: number; y: number; w: number; h: number }
    },
  ]
}>()

const visible = defineModel<boolean>('visible', { default: false })
const datasources = ref<Datasource[]>([])

const panelTypes = [
  { label: 'Time Series', value: 'timeseries' },
  { label: 'Stat', value: 'stat' },
  { label: 'Gauge', value: 'gauge' },
  { label: 'Table', value: 'table' },
  { label: 'Bar Chart', value: 'bar' },
  { label: 'Heatmap', value: 'heatmap' },
  { label: 'Pie Chart', value: 'piechart' },
  { label: 'Logs', value: 'logs' },
]

const form = reactive({
  title: '',
  type: 'timeseries',
  datasource_id: '',
  query: '',
  x: 0,
  y: 0,
  w: 6,
  h: 3,
})

const queryLanguage = computed(() => {
  const ds = datasources.value.find((d) => d.id === form.datasource_id)
  if (ds?.type === 'postgresql') return 'sql'
  return 'promql'
})

watch(
  () => props.panel,
  (p) => {
    if (p) {
      form.title = p.title
      form.type = p.type
      form.datasource_id = p.datasource_id || ''
      form.query = p.query
      form.x = p.position.x
      form.y = p.position.y
      form.w = p.position.w
      form.h = p.position.h
    } else {
      form.title = ''
      form.type = 'timeseries'
      form.datasource_id = ''
      form.query = ''
      form.x = 0
      form.y = 0
      form.w = 6
      form.h = 3
    }
  },
  { immediate: true },
)

function save() {
  emit('save', {
    title: form.title,
    type: form.type,
    datasource_id: form.datasource_id,
    query: form.query,
    position: { x: form.x, y: form.y, w: form.w, h: form.h },
  })
  visible.value = false
}

onMounted(async () => {
  datasources.value = await datasourcesApi.list()
})
</script>
