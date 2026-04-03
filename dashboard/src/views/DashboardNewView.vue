<template>
  <div class="max-w-2xl">
    <h1 class="text-2xl font-bold mb-6">New Dashboard</h1>

    <div class="flex gap-4 mb-6">
      <button
        class="btn"
        :class="mode === 'blank' ? 'btn-primary' : 'btn-ghost'"
        @click="mode = 'blank'"
      >
        Blank Dashboard
      </button>
      <button
        class="btn"
        :class="mode === 'template' ? 'btn-primary' : 'btn-ghost'"
        @click="mode = 'template'"
      >
        From Template
      </button>
    </div>

    <!-- Blank mode -->
    <form v-if="mode === 'blank'" @submit.prevent="createBlank" class="space-y-4">
      <div class="form-control">
        <label class="label">Title</label>
        <InputText v-model="form.title" class="w-full" required />
      </div>
      <div class="form-control">
        <label class="label">Slug</label>
        <InputText v-model="form.slug" class="w-full" placeholder="my-dashboard" required />
        <span class="text-xs text-base-content/50 mt-1">URL-friendly identifier</span>
      </div>
      <div class="form-control">
        <label class="label">Description</label>
        <InputText v-model="form.description" class="w-full" />
      </div>
      <div class="flex gap-2">
        <button type="submit" class="btn btn-primary">Create</button>
        <RouterLink to="/dashboards" class="btn btn-ghost">Cancel</RouterLink>
      </div>
    </form>

    <!-- Template mode -->
    <div v-else>
      <p class="text-base-content/60 mb-4">Choose a template to get started quickly.</p>
      <RouterLink to="/templates" class="btn btn-primary">
        <i class="pi pi-copy mr-2" /> Browse Templates
      </RouterLink>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { dashboardsApi } from '@/api/dashboards'
import InputText from 'primevue/inputtext'

const router = useRouter()
const mode = ref<'blank' | 'template'>('blank')

const form = reactive({
  title: '',
  slug: '',
  description: '',
})

watch(
  () => form.title,
  (title) => {
    form.slug = title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '')
  },
)

async function createBlank() {
  const dash = await dashboardsApi.create(form)
  router.push(`/dashboards/${dash.slug}/edit`)
}
</script>
