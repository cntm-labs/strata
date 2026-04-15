import { ref, computed } from 'vue'

interface User {
  id: string
  email: string
  firstName?: string
  lastName?: string
  avatarUrl?: string
}

const token = ref<string | null>(null)
const user = ref<User | null>(null)

export function useAuth() {
  const isAuthenticated = computed(() => !!token.value)

  function setToken(jwt: string) {
    token.value = jwt
    try {
      const payloadPart = jwt.split('.')[1]
      if (!payloadPart) throw new Error('Invalid JWT')
      const payload = JSON.parse(atob(payloadPart))
      user.value = {
        id: payload.sub,
        email: payload.email,
        firstName: payload.first_name,
        lastName: payload.last_name,
        avatarUrl: payload.avatar_url,
      }
    } catch {
      user.value = null
    }
  }

  function clearToken() {
    token.value = null
    user.value = null
  }

  function getToken(): string | null {
    return token.value
  }

  return { token, user, isAuthenticated, setToken, clearToken, getToken }
}
