<template>
  <div class="max-w-2xl">
    <h1 class="text-2xl font-bold mb-6">Settings</h1>

    <form @submit.prevent="save" class="space-y-4">
      <div class="form-control">
        <label class="label">Theme</label>
        <Select
          v-model="form.theme"
          :options="themeOptions"
          optionLabel="label"
          optionValue="value"
          class="w-full"
        />
      </div>

      <div class="form-control">
        <label class="label">Timezone</label>
        <InputText v-model="form.timezone" class="w-full" />
      </div>

      <div class="form-control">
        <label class="label">Default Dashboard</label>
        <Select
          v-model="form.default_dashboard_id"
          :options="dashboards"
          optionLabel="title"
          optionValue="id"
          class="w-full"
          showClear
          placeholder="None"
        />
      </div>

      <button type="submit" class="btn btn-primary">Save</button>
      <span v-if="saved" class="text-success ml-2">Saved!</span>
    </form>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, watch } from "vue";
import { dashboardsApi } from "@/api/dashboards";
import InputText from "primevue/inputtext";
import Select from "primevue/select";
import type { Dashboard } from "@/types";

const dashboards = ref<Dashboard[]>([]);
const saved = ref(false);

const themeOptions = [
  { label: "System", value: "system" },
  { label: "Light", value: "light" },
  { label: "Dark", value: "dark" },
];

const form = reactive({
  theme: "system",
  timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
  default_dashboard_id: "",
});

// Apply theme to document
watch(
  () => form.theme,
  (theme) => {
    if (theme === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
  },
);

function save() {
  // Save to localStorage for now (backend user preferences requires auth)
  localStorage.setItem("strata-settings", JSON.stringify(form));
  saved.value = true;
  setTimeout(() => {
    saved.value = false;
  }, 2000);
}

onMounted(async () => {
  dashboards.value = await dashboardsApi.list();
  const stored = localStorage.getItem("strata-settings");
  if (stored) {
    Object.assign(form, JSON.parse(stored));
  }
});
</script>
