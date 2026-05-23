import { createApp } from 'vue'
import { createPinia } from 'pinia'
import PrimeVue from 'primevue/config'
import Aura from '@primevue/themes/aura'
import { Nucleus } from '@cntm-labs/nucleus-js'
import 'primeicons/primeicons.css'
import './assets/main.css'

import App from './App.vue'
import router from './router'

async function bootstrap() {
  const authEnabled = import.meta.env.VITE_AUTH_ENABLED === 'true'

  if (authEnabled) {
    const publishableKey = import.meta.env.VITE_NUCLEUS_PUBLISHABLE_KEY
    const baseUrl = import.meta.env.VITE_NUCLEUS_BASE_URL
    if (!publishableKey) {
      console.error(
        '[strata] VITE_AUTH_ENABLED=true but VITE_NUCLEUS_PUBLISHABLE_KEY is unset — sign-in will not work',
      )
    } else {
      await Nucleus.configure({ publishableKey, baseUrl })
    }
  }

  const app = createApp(App)
  app.use(createPinia())
  app.use(router)
  app.use(PrimeVue, {
    theme: {
      preset: Aura,
    },
  })
  app.mount('#app')
}

bootstrap()
