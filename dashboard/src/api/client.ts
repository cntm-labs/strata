import { Nucleus } from '@cntm-labs/nucleus-js'
import router from '@/router'

const BASE_URL = '/api/v1'

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' }
  const jwt = Nucleus.getToken()
  if (jwt) {
    headers['Authorization'] = `Bearer ${jwt}`
  }

  const res = await fetch(`${BASE_URL}${path}`, {
    headers,
    ...options,
  })

  if (res.status === 401) {
    await Nucleus.signOut()
    router.push({ name: 'login' })
    throw new Error('Unauthorized')
  }

  if (!res.ok) {
    const error = await res.json().catch(() => ({ message: res.statusText }))
    throw new Error(error.message || `HTTP ${res.status}`)
  }
  return res.json()
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'POST',
      body: body ? JSON.stringify(body) : undefined,
    }),
  put: <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'PUT',
      body: body ? JSON.stringify(body) : undefined,
    }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
}
