<template>
  <div>
    <h1 class="text-2xl font-bold mb-6">Dashboard Templates</h1>

    <div v-if="loading" class="flex justify-center p-8">
      <ProgressSpinner />
    </div>

    <template v-else>
      <div v-for="category in categories" :key="category" class="mb-8">
        <h2 class="text-lg font-semibold mb-3 capitalize">{{ category }}</h2>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <div
            v-for="tpl in templatesByCategory(category)"
            :key="tpl.id"
            class="card bg-base-200 shadow-sm hover:shadow-md transition cursor-pointer"
            @click="useTemplate(tpl)"
          >
            <div class="card-body p-4">
              <h3 class="card-title text-lg">{{ tpl.name }}</h3>
              <p class="text-sm text-base-content/60">{{ tpl.description }}</p>
              <div class="flex gap-2 mt-2">
                <span
                  v-if="tpl.required_datasource_type"
                  class="badge badge-sm badge-outline"
                >
                  {{ tpl.required_datasource_type }}
                </span>
                <span class="badge badge-sm badge-ghost"
                  >{{ panelCount(tpl) }} panels</span
                >
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- Use template dialog -->
    <Dialog
      v-model:visible="dialogVisible"
      header="Create from Template"
      :style="{ width: '400px' }"
      modal
    >
      <form @submit.prevent="confirmUse" class="space-y-4">
        <div class="form-control">
          <label class="label">Dashboard Title</label>
          <InputText v-model="newTitle" class="w-full" required />
        </div>
        <div class="form-control">
          <label class="label">Slug</label>
          <InputText v-model="newSlug" class="w-full" required />
        </div>
        <div
          v-if="selectedTemplate?.required_datasource_type"
          class="form-control"
        >
          <label class="label">Datasource</label>
          <Select
            v-model="selectedDatasourceId"
            :options="filteredDatasources"
            optionLabel="name"
            optionValue="id"
            class="w-full"
            placeholder="Select datasource"
          />
        </div>
        <div class="flex gap-2 justify-end">
          <button
            type="button"
            class="btn btn-ghost"
            @click="dialogVisible = false"
          >
            Cancel
          </button>
          <button type="submit" class="btn btn-primary">Create</button>
        </div>
      </form>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { useRouter } from "vue-router";
import { templatesApi, type DashboardTemplate } from "@/api/templates";
import { datasourcesApi } from "@/api/datasources";
import Dialog from "primevue/dialog";
import InputText from "primevue/inputtext";
import Select from "primevue/select";
import ProgressSpinner from "primevue/progressspinner";
import type { Datasource } from "@/types";

const router = useRouter();
const templates = ref<DashboardTemplate[]>([]);
const datasources = ref<Datasource[]>([]);
const loading = ref(true);

const dialogVisible = ref(false);
const selectedTemplate = ref<DashboardTemplate | null>(null);
const newTitle = ref("");
const newSlug = ref("");
const selectedDatasourceId = ref("");

const categories = computed(() => [
  ...new Set(templates.value.map((t) => t.category)),
]);

function templatesByCategory(cat: string) {
  return templates.value.filter((t) => t.category === cat);
}

function panelCount(tpl: DashboardTemplate) {
  const json = tpl.dashboard_json as any;
  return json?.panels?.length || 0;
}

const filteredDatasources = computed(() => {
  if (!selectedTemplate.value?.required_datasource_type)
    return datasources.value;
  return datasources.value.filter(
    (d) => d.type === selectedTemplate.value!.required_datasource_type,
  );
});

function useTemplate(tpl: DashboardTemplate) {
  selectedTemplate.value = tpl;
  newTitle.value = tpl.name;
  newSlug.value = tpl.slug + "-" + Date.now().toString(36);
  selectedDatasourceId.value = "";
  dialogVisible.value = true;
}

watch(
  () => newTitle.value,
  (title) => {
    newSlug.value = title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
  },
);

async function confirmUse() {
  if (!selectedTemplate.value) return;
  const dash = await templatesApi.use(selectedTemplate.value.slug, {
    title: newTitle.value,
    slug: newSlug.value,
    datasource_id: selectedDatasourceId.value || undefined,
  });
  dialogVisible.value = false;
  router.push(`/dashboards/${dash.slug}`);
}

onMounted(async () => {
  try {
    const [tpls, dss] = await Promise.all([
      templatesApi.list(),
      datasourcesApi.list(),
    ]);
    templates.value = tpls;
    datasources.value = dss;
  } finally {
    loading.value = false;
  }
});
</script>
