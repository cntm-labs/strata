<template>
  <div class="max-w-2xl">
    <h1 class="text-2xl font-bold mb-6">{{ isNew ? 'Add' : 'Edit' }} Data Source</h1>

    <form @submit.prevent="save" class="space-y-4">
      <div class="form-control">
        <label class="label">Name</label>
        <InputText v-model="form.name" class="w-full" required />
      </div>

      <div class="form-control">
        <label class="label">Type</label>
        <Select
          v-model="form.type"
          :options="typeOptions"
          optionLabel="label"
          optionValue="value"
          class="w-full"
        />
      </div>

      <div class="form-control">
        <label class="label">URL</label>
        <InputText v-model="form.url" class="w-full" :placeholder="urlPlaceholder" required />
      </div>

      <div class="form-control">
        <label class="label">Credentials (optional)</label>
        <InputText v-model="form.credentials" class="w-full" type="password" />
      </div>

      <div class="form-control">
        <label class="label cursor-pointer justify-start gap-2">
          <input type="checkbox" v-model="form.is_default" class="checkbox" />
          <span>Set as default</span>
        </label>
      </div>

      <div class="flex gap-2">
        <button type="submit" class="btn btn-primary">Save</button>
        <RouterLink to="/datasources" class="btn btn-ghost">Cancel</RouterLink>
      </div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { datasourcesApi } from '@/api/datasources'
import InputText from 'primevue/inputtext'
import Select from 'primevue/select'

const route = useRoute()
const router = useRouter()
const isNew = computed(() => route.name === 'datasource-new')

const typeOptions = [
  { label: 'Prometheus', value: 'prometheus' },
  { label: 'Loki', value: 'loki' },
  { label: 'PostgreSQL', value: 'postgresql' },
]

const form = reactive({
  name: '',
  type: 'prometheus' as 'prometheus' | 'loki' | 'postgresql',
  url: '',
  credentials: '',
  is_default: false,
})

const urlPlaceholder = computed(() => {
  const placeholders: Record<string, string> = {
    prometheus: 'http://prometheus:9090',
    loki: 'http://loki:3100',
    postgresql: 'postgres://user:pass@host:5432/db',
  }
  return placeholders[form.type] || ''
})

async function save() {
  if (isNew.value) {
    await datasourcesApi.create(form)
  } else {
    await datasourcesApi.update(route.params.id as string, form)
  }
  router.push('/datasources')
}

onMounted(async () => {
  if (!isNew.value) {
    const data = await datasourcesApi.get(route.params.id as string)
    Object.assign(form, data)
  }
})
</script>
