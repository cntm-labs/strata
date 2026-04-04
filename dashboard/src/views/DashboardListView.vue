<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Dashboards</h1>
      <RouterLink to="/dashboards/new" class="btn btn-primary">
        <i class="pi pi-plus mr-2" /> New Dashboard
      </RouterLink>
    </div>

    <InputText v-model="search" placeholder="Search dashboards..." class="w-full mb-6" />

    <div v-if="loading" class="flex justify-center p-8">
      <ProgressSpinner />
    </div>

    <template v-else>
      <!-- Starred dashboards as cards -->
      <div v-if="starred.length > 0" class="mb-8">
        <h2 class="text-lg font-semibold mb-3">Starred</h2>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <div
            v-for="dash in starred"
            :key="dash.id"
            class="card bg-base-200 shadow-sm hover:shadow-md transition cursor-pointer"
            @click="$router.push(`/dashboards/${dash.slug}`)"
          >
            <div class="card-body p-4">
              <div class="flex items-center justify-between">
                <h3 class="card-title text-lg">{{ dash.title }}</h3>
                <button class="btn btn-ghost btn-sm" @click.stop="toggleStar(dash.slug)">
                  <i class="pi pi-star-fill text-warning" />
                </button>
              </div>
              <p class="text-sm text-base-content/60">
                {{ dash.description || 'No description' }}
              </p>
              <div class="text-xs text-base-content/40 mt-2">
                Updated {{ formatDate(dash.updated_at) }}
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Rest as table -->
      <div v-if="unstarred.length > 0">
        <h2 v-if="starred.length > 0" class="text-lg font-semibold mb-3">All Dashboards</h2>
        <DataTable :value="unstarred" stripedRows class="rounded-lg overflow-hidden">
          <Column header="Name">
            <template #body="{ data }">
              <RouterLink :to="`/dashboards/${data.slug}`" class="link font-medium">
                {{ data.title }}
              </RouterLink>
            </template>
          </Column>
          <Column field="description" header="Description">
            <template #body="{ data }">
              <span class="text-base-content/60">{{ data.description || '—' }}</span>
            </template>
          </Column>
          <Column header="Updated" style="width: 140px">
            <template #body="{ data }">
              <span class="text-sm text-base-content/50">{{ formatDate(data.updated_at) }}</span>
            </template>
          </Column>
          <Column header="" style="width: 60px">
            <template #body="{ data }">
              <button class="btn btn-ghost btn-sm" @click="toggleStar(data.slug)">
                <i class="pi pi-star" />
              </button>
            </template>
          </Column>
        </DataTable>
      </div>

      <div v-if="filtered.length === 0" class="text-center p-12 text-base-content/60">
        <template v-if="search">No dashboards matching "{{ search }}"</template>
        <template v-else>
          No dashboards yet.
          <RouterLink to="/dashboards/new" class="link">Create one</RouterLink>
        </template>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useDashboardStore } from '@/stores/dashboards'
import { dashboardsApi } from '@/api/dashboards'
import InputText from 'primevue/inputtext'
import ProgressSpinner from 'primevue/progressspinner'
import DataTable from 'primevue/datatable'
import Column from 'primevue/column'

const store = useDashboardStore()
const search = ref('')

const loading = computed(() => store.loading)

const filtered = computed(() =>
  store.items.filter((d) => d.title.toLowerCase().includes(search.value.toLowerCase())),
)

const starred = computed(() => filtered.value.filter((d) => d.is_starred))
const unstarred = computed(() => filtered.value.filter((d) => !d.is_starred))

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString()
}

async function toggleStar(slug: string) {
  await dashboardsApi.toggleStar(slug)
  await store.fetchAll()
}

onMounted(() => store.fetchAll())
</script>
