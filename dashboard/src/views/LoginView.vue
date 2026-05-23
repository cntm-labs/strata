<template>
  <div class="flex items-center justify-center min-h-screen bg-base-200">
    <Card class="w-full max-w-md">
      <template #title>Sign in to Strata</template>
      <template #content>
        <form data-test="email-form" class="flex flex-col gap-3" @submit.prevent="onEmailSignIn">
          <InputText
            v-model="email"
            type="email"
            placeholder="Email"
            required
            autocomplete="email"
            data-test="email-input"
          />
          <Password
            v-model="password"
            placeholder="Password"
            :feedback="false"
            toggle-mask
            required
            autocomplete="current-password"
            input-class="w-full"
            class="w-full"
            data-test="password-input"
          />
          <Button
            type="submit"
            label="Sign in"
            :loading="submitting"
            :disabled="submitting"
            data-test="submit-btn"
          />
          <p v-if="error" data-test="error" class="text-red-500 text-sm">{{ error }}</p>
        </form>

        <Divider align="center"><span>or</span></Divider>

        <div class="flex flex-col gap-2">
          <Button
            label="Continue with Google"
            icon="pi pi-google"
            outlined
            :disabled="submitting"
            data-test="oauth-google"
            @click="onOAuth('google')"
          />
          <Button
            label="Continue with GitHub"
            icon="pi pi-github"
            outlined
            :disabled="submitting"
            data-test="oauth-github"
            @click="onOAuth('github')"
          />
        </div>
      </template>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { Nucleus } from '@cntm-labs/nucleus-js'
import Card from 'primevue/card'
import InputText from 'primevue/inputtext'
import Password from 'primevue/password'
import Button from 'primevue/button'
import Divider from 'primevue/divider'

const router = useRouter()
const email = ref('')
const password = ref('')
const submitting = ref(false)
const error = ref<string | null>(null)

async function onEmailSignIn() {
  submitting.value = true
  error.value = null
  try {
    await Nucleus.signIn(email.value, password.value)
    router.replace('/dashboards')
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Sign-in failed'
  } finally {
    submitting.value = false
  }
}

async function onOAuth(provider: 'google' | 'github') {
  submitting.value = true
  error.value = null
  try {
    await Nucleus.signInWithOAuth(provider)
    router.replace('/dashboards')
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'OAuth failed'
  } finally {
    submitting.value = false
  }
}
</script>
