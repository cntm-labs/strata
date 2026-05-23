import { ref, computed } from 'vue'
import { Nucleus } from '@cntm-labs/nucleus-js'

const tick = ref(0)
let subscribed = false

function ensureSubscribed() {
  if (subscribed) return
  Nucleus.addListener(() => {
    tick.value++
  })
  subscribed = true
}

export function useAuth() {
  ensureSubscribed()

  const user = computed(() => {
    void tick.value
    return Nucleus.user
  })
  const organization = computed(() => {
    void tick.value
    return Nucleus.organization
  })
  const isAuthenticated = computed(() => {
    void tick.value
    return Nucleus.isSignedIn
  })

  function getToken(): string | null {
    return Nucleus.getToken()
  }

  async function signOut(): Promise<void> {
    await Nucleus.signOut()
  }

  return { user, organization, isAuthenticated, getToken, signOut }
}
