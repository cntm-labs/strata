<template>
  <div class="max-w-2xl">
    <h1 class="text-2xl font-bold mb-6">
      {{ isNew ? "New" : "Edit" }} Alert Rule
    </h1>

    <form @submit.prevent="save" class="space-y-4">
      <div class="form-control">
        <label class="label">Name</label>
        <InputText v-model="form.name" class="w-full" required />
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
        <MonacoEditor v-model="form.query" :height="100" />
      </div>

      <div class="grid grid-cols-3 gap-4">
        <div class="form-control">
          <label class="label">Condition</label>
          <Select
            v-model="form.condition"
            :options="conditionOptions"
            optionLabel="label"
            optionValue="value"
            class="w-full"
          />
        </div>
        <div class="form-control">
          <label class="label">Threshold</label>
          <InputNumber v-model="form.threshold" class="w-full" />
        </div>
        <div class="form-control">
          <label class="label">Duration (sec)</label>
          <InputNumber v-model="form.duration_secs" class="w-full" />
        </div>
      </div>

      <div class="form-control">
        <label class="label">Severity</label>
        <Select
          v-model="form.severity"
          :options="severityOptions"
          optionLabel="label"
          optionValue="value"
          class="w-full"
        />
      </div>

      <div class="flex gap-2">
        <button type="submit" class="btn btn-primary">Save</button>
        <RouterLink to="/alerts" class="btn btn-ghost">Cancel</RouterLink>
      </div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { alertsApi } from "@/api/alerts";
import { datasourcesApi } from "@/api/datasources";
import MonacoEditor from "@/components/MonacoEditor.vue";
import InputText from "primevue/inputtext";
import InputNumber from "primevue/inputnumber";
import Select from "primevue/select";
import type { Datasource } from "@/types";

const route = useRoute();
const router = useRouter();
const isNew = computed(() => route.name === "alert-rule-new");
const datasources = ref<Datasource[]>([]);

const conditionOptions = [
  { label: "> (greater than)", value: "gt" },
  { label: "< (less than)", value: "lt" },
  { label: "= (equal)", value: "eq" },
  { label: ">= (greater or equal)", value: "gte" },
  { label: "<= (less or equal)", value: "lte" },
];

const severityOptions = [
  { label: "Info", value: "info" },
  { label: "Warning", value: "warning" },
  { label: "Critical", value: "critical" },
];

const form = reactive({
  name: "",
  datasource_id: "",
  query: "",
  condition: "gt" as "gt" | "lt" | "eq" | "gte" | "lte",
  threshold: 0,
  duration_secs: 60,
  severity: "warning" as "info" | "warning" | "critical",
  notification_channels: [] as string[],
  notification_recipients: [] as string[],
});

async function save() {
  if (isNew.value) {
    await alertsApi.createRule(form);
  } else {
    await alertsApi.updateRule(route.params.id as string, form);
  }
  router.push("/alerts");
}

onMounted(async () => {
  datasources.value = await datasourcesApi.list();
  if (!isNew.value) {
    const rule = await alertsApi.getRule(route.params.id as string);
    Object.assign(form, rule);
  }
});
</script>
