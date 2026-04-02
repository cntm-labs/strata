import { defineStore } from "pinia";
import { ref } from "vue";
import { datasourcesApi } from "@/api/datasources";
import type { Datasource } from "@/types";

export const useDatasourceStore = defineStore("datasources", () => {
  const items = ref<Datasource[]>([]);
  const loading = ref(false);

  async function fetchAll() {
    loading.value = true;
    try {
      items.value = await datasourcesApi.list();
    } finally {
      loading.value = false;
    }
  }

  return { items, loading, fetchAll };
});
