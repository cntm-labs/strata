<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Data Sources</h1>
      <RouterLink to="/datasources/new" class="btn btn-primary">
        <i class="pi pi-plus mr-2" /> Add Data Source
      </RouterLink>
    </div>

    <div v-if="store.loading" class="flex justify-center p-8">
      <ProgressSpinner />
    </div>

    <DataTable
      v-else
      :value="store.items"
      stripedRows
      class="rounded-lg overflow-hidden"
    >
      <Column field="name" header="Name" />
      <Column field="type" header="Type">
        <template #body="{ data }">
          <span class="badge" :class="typeBadgeClass(data.type)">{{
            data.type
          }}</span>
        </template>
      </Column>
      <Column field="url" header="URL" />
      <Column field="is_default" header="Default" style="width: 80px">
        <template #body="{ data }">
          <i v-if="data.is_default" class="pi pi-check text-success" />
        </template>
      </Column>
      <Column header="Actions" style="width: 200px">
        <template #body="{ data }">
          <div class="flex gap-2">
            <button class="btn btn-sm btn-ghost" @click="testDs(data.id)">
              Test
            </button>
            <RouterLink
              :to="`/datasources/${data.id}`"
              class="btn btn-sm btn-ghost"
              >Edit</RouterLink
            >
            <button
              class="btn btn-sm btn-ghost text-error"
              @click="removeDs(data.id)"
            >
              Delete
            </button>
          </div>
        </template>
      </Column>
    </DataTable>

    <div
      v-if="!store.loading && store.items.length === 0"
      class="text-center p-12 text-base-content/60"
    >
      No data sources configured.
      <RouterLink to="/datasources/new" class="link">Add one</RouterLink>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { useDatasourceStore } from "@/stores/datasources";
import { datasourcesApi } from "@/api/datasources";
import DataTable from "primevue/datatable";
import Column from "primevue/column";
import ProgressSpinner from "primevue/progressspinner";

const store = useDatasourceStore();

function typeBadgeClass(type: string) {
  return (
    {
      prometheus: "badge-primary",
      loki: "badge-secondary",
      postgresql: "badge-accent",
    }[type] || "badge-ghost"
  );
}

async function testDs(id: string) {
  const result = await datasourcesApi.test(id);
  alert(result.success ? "Connection successful!" : "Connection failed");
}

async function removeDs(id: string) {
  if (confirm("Delete this datasource?")) {
    await datasourcesApi.remove(id);
    await store.fetchAll();
  }
}

onMounted(() => store.fetchAll());
</script>
