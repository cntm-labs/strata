<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <div>
        <h1 class="text-2xl font-bold">Edit: {{ dashboard?.title }}</h1>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-primary" @click="addPanel">
          <i class="pi pi-plus mr-1" /> Add Panel
        </button>
        <RouterLink :to="`/dashboards/${slug}`" class="btn btn-ghost">
          <i class="pi pi-eye mr-1" /> View
        </RouterLink>
      </div>
    </div>

    <grid-layout
      v-if="panels.length > 0"
      :layout="gridLayout"
      :col-num="12"
      :row-height="80"
      :is-draggable="true"
      :is-resizable="true"
      @layout-updated="onLayoutUpdated"
    >
      <grid-item
        v-for="panel in panels"
        :key="panel.id"
        :x="panel.position.x"
        :y="panel.position.y"
        :w="panel.position.w"
        :h="panel.position.h"
        :i="panel.id"
      >
        <PanelRenderer
          :panel="panel"
          :data="panelData[panel.id]"
          :editable="true"
          @edit="editPanel"
        />
        <button
          class="btn btn-xs btn-error btn-circle absolute top-1 right-1 z-10"
          @click="removePanel(panel.id)"
        >
          <i class="pi pi-times text-xs" />
        </button>
      </grid-item>
    </grid-layout>

    <div v-else class="text-center p-12 text-base-content/60">
      No panels. Click "Add Panel" to get started.
    </div>

    <PanelEditor v-model:visible="editorVisible" :panel="editingPanel" @save="onPanelSave" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, reactive } from 'vue'
import { useRoute } from 'vue-router'
import { dashboardsApi } from '@/api/dashboards'
import { datasourcesApi } from '@/api/datasources'
import PanelRenderer from '@/components/panels/PanelRenderer.vue'
import PanelEditor from '@/components/PanelEditor.vue'
import { GridItem } from 'grid-layout-plus'
import type { Dashboard, Panel, PanelType } from '@/types'

const route = useRoute()
const slug = computed(() => route.params.slug as string)
const dashboard = ref<Dashboard | null>(null)
const panels = ref<Panel[]>([])
const panelData = reactive<Record<string, unknown>>({})

const editorVisible = ref(false)
const editingPanel = ref<Panel | undefined>(undefined)

const gridLayout = computed(() => panels.value.map((p) => ({ ...p.position, i: p.id })))

function addPanel() {
  editingPanel.value = undefined
  editorVisible.value = true
}

function editPanel(panel: Panel) {
  editingPanel.value = panel
  editorVisible.value = true
}

async function onPanelSave(data: {
  title: string
  type: string
  datasource_id: string
  query: string
  position: { x: number; y: number; w: number; h: number }
}) {
  const panelPayload = {
    title: data.title,
    type: data.type as PanelType,
    datasource_id: data.datasource_id,
    query: data.query,
    config: {},
    position: { ...data.position, i: '' },
  }

  if (editingPanel.value) {
    await dashboardsApi.updatePanel(editingPanel.value.id, panelPayload)
  } else {
    await dashboardsApi.addPanel(slug.value, panelPayload)
  }
  await reload()
}

async function removePanel(id: string) {
  if (confirm('Remove this panel?')) {
    await dashboardsApi.removePanel(id)
    await reload()
  }
}

async function onLayoutUpdated(
  layout: Array<{ i: string; x: number; y: number; w: number; h: number }>,
) {
  for (const item of layout) {
    const panel = panels.value.find((p) => p.id === item.i)
    if (!panel) continue
    const posChanged =
      panel.position.x !== item.x ||
      panel.position.y !== item.y ||
      panel.position.w !== item.w ||
      panel.position.h !== item.h
    if (posChanged) {
      await dashboardsApi.updatePanel(panel.id, {
        position: { x: item.x, y: item.y, w: item.w, h: item.h, i: item.i },
      })
    }
  }
}

async function reload() {
  const p = await dashboardsApi.listPanels(slug.value)
  panels.value = p
  for (const panel of p) {
    if (panel.datasource_id) {
      const now = Math.floor(Date.now() / 1000)
      panelData[panel.id] = await datasourcesApi.query(panel.datasource_id, {
        query: panel.query,
        start: (now - 3600).toString(),
        end: now.toString(),
        step: '15',
      })
    }
  }
}

onMounted(async () => {
  const dash = await dashboardsApi.get(slug.value)
  dashboard.value = dash
  await reload()
})
</script>
