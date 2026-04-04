<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Alerts</h1>
      <RouterLink to="/alerts/rules/new" class="btn btn-primary">
        <i class="pi pi-plus mr-2" /> New Rule
      </RouterLink>
    </div>

    <div class="tabs tabs-bordered mb-4">
      <a class="tab" :class="{ 'tab-active': tab === 'rules' }" @click="tab = 'rules'">Rules</a>
      <a class="tab" :class="{ 'tab-active': tab === 'events' }" @click="tab = 'events'">Events</a>
    </div>

    <!-- Rules tab -->
    <div v-if="tab === 'rules'">
      <div v-if="loadingRules" class="flex justify-center p-8">
        <ProgressSpinner />
      </div>
      <DataTable v-else :value="rules" stripedRows class="rounded-lg overflow-hidden">
        <Column field="name" header="Name">
          <template #body="{ data }">
            <RouterLink :to="`/alerts/rules/${data.id}`" class="link font-medium">
              {{ data.name }}
            </RouterLink>
          </template>
        </Column>
        <Column header="Condition">
          <template #body="{ data }">
            <span class="font-mono text-sm">{{ data.condition }} {{ data.threshold }}</span>
          </template>
        </Column>
        <Column field="severity" header="Severity">
          <template #body="{ data }">
            <span class="badge" :class="severityClass(data.severity)">{{ data.severity }}</span>
          </template>
        </Column>
        <Column header="State">
          <template #body="{ data }">
            <span class="badge" :class="stateClass(data.current_state)">{{
              data.current_state || 'ok'
            }}</span>
          </template>
        </Column>
        <Column header="Active" style="width: 70px">
          <template #body="{ data }">
            <input
              type="checkbox"
              class="toggle toggle-sm"
              :checked="data.is_active"
              @change="toggleActive(data)"
            />
          </template>
        </Column>
        <Column header="" style="width: 80px">
          <template #body="{ data }">
            <button class="btn btn-sm btn-ghost text-error" @click="deleteRule(data.id)">
              Delete
            </button>
          </template>
        </Column>
      </DataTable>
    </div>

    <!-- Events tab -->
    <div v-if="tab === 'events'">
      <div v-if="loadingEvents" class="flex justify-center p-8">
        <ProgressSpinner />
      </div>
      <DataTable v-else :value="events" stripedRows class="rounded-lg overflow-hidden">
        <Column header="State">
          <template #body="{ data }">
            <span class="badge" :class="stateClass(data.state)">{{ data.state }}</span>
          </template>
        </Column>
        <Column field="value" header="Value">
          <template #body="{ data }">
            <span class="font-mono text-sm">{{ data.value?.toFixed(2) ?? '—' }}</span>
          </template>
        </Column>
        <Column field="message" header="Message">
          <template #body="{ data }">
            <span class="text-sm">{{ data.message || '—' }}</span>
          </template>
        </Column>
        <Column header="Time">
          <template #body="{ data }">
            <span class="text-sm text-base-content/50">{{ formatTime(data.created_at) }}</span>
          </template>
        </Column>
      </DataTable>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { alertsApi } from '@/api/alerts'
import DataTable from 'primevue/datatable'
import Column from 'primevue/column'
import ProgressSpinner from 'primevue/progressspinner'
import type { AlertRule, AlertEvent } from '@/types'

const tab = ref<'rules' | 'events'>('rules')
const rules = ref<AlertRule[]>([])
const events = ref<AlertEvent[]>([])
const loadingRules = ref(true)
const loadingEvents = ref(true)

function severityClass(severity: string) {
  return (
    { info: 'badge-info', warning: 'badge-warning', critical: 'badge-error' }[severity] ||
    'badge-ghost'
  )
}

function stateClass(state: string) {
  return (
    { ok: 'badge-success', firing: 'badge-error', pending: 'badge-warning' }[state] || 'badge-ghost'
  )
}

function formatTime(iso: string) {
  return new Date(iso).toLocaleString()
}

async function toggleActive(rule: AlertRule) {
  await alertsApi.updateRule(rule.id, { is_active: !rule.is_active })
  await loadRules()
}

async function deleteRule(id: string) {
  if (confirm('Delete this alert rule?')) {
    await alertsApi.deleteRule(id)
    await loadRules()
  }
}

async function loadRules() {
  loadingRules.value = true
  try {
    rules.value = await alertsApi.listRules()
  } finally {
    loadingRules.value = false
  }
}

async function loadEvents() {
  loadingEvents.value = true
  try {
    events.value = await alertsApi.listEvents({ limit: 100 })
  } finally {
    loadingEvents.value = false
  }
}

onMounted(() => {
  loadRules()
  loadEvents()
})
</script>
