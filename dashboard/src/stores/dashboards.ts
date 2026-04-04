import { defineStore } from 'pinia'
import { ref } from 'vue'
import { dashboardsApi } from '@/api/dashboards'
import type { Dashboard } from '@/types'

export const useDashboardStore = defineStore('dashboards', () => {
  const items = ref<Dashboard[]>([])
  const loading = ref(false)

  async function fetchAll() {
    loading.value = true
    try {
      items.value = await dashboardsApi.list()
    } finally {
      loading.value = false
    }
  }

  return { items, loading, fetchAll }
})
