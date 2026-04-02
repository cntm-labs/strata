<template>
  <AgGridVue
    class="ag-theme-alpine-dark w-full h-full"
    :rowData="rowData"
    :columnDefs="columnDefs"
    :defaultColDef="{ sortable: true, filter: true, resizable: true }"
  />
</template>

<script setup lang="ts">
import { computed } from "vue";
import { AgGridVue } from "ag-grid-vue3";

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>();

const rowData = computed(() => {
  const result = (props.data as any)?.data?.result || props.data;
  if (Array.isArray(result)) return result;
  return [];
});

const columnDefs = computed(() => {
  const first = rowData.value[0];
  if (!first) return [];
  return Object.keys(first).map((key) => ({ field: key, headerName: key }));
});
</script>
