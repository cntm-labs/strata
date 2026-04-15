import { describe, it, expect, beforeEach } from 'vitest'
import { useAuth } from '../useAuth'

function makeJwt(payload: Record<string, unknown>): string {
  const header = btoa(JSON.stringify({ alg: 'ES256' }))
  const body = btoa(JSON.stringify(payload))
  return `${header}.${body}.fakesignature`
}

describe('useAuth', () => {
  beforeEach(() => {
    useAuth().clearToken()
  })

  it('starts unauthenticated', () => {
    const { isAuthenticated, token, user, getToken } = useAuth()
    expect(isAuthenticated.value).toBe(false)
    expect(token.value).toBeNull()
    expect(user.value).toBeNull()
    expect(getToken()).toBeNull()
  })

  it('setToken stores token and decodes user', () => {
    const { setToken, isAuthenticated, token, user, getToken } = useAuth()
    const jwt = makeJwt({
      sub: 'user-123',
      email: 'alice@test.com',
      first_name: 'Alice',
      last_name: 'Wonder',
      avatar_url: 'https://img.test/alice.png',
    })

    setToken(jwt)

    expect(isAuthenticated.value).toBe(true)
    expect(token.value).toBe(jwt)
    expect(getToken()).toBe(jwt)
    expect(user.value).toEqual({
      id: 'user-123',
      email: 'alice@test.com',
      firstName: 'Alice',
      lastName: 'Wonder',
      avatarUrl: 'https://img.test/alice.png',
    })
  })

  it('setToken handles missing optional fields', () => {
    const { setToken, user } = useAuth()
    const jwt = makeJwt({ sub: 'user-456', email: 'bob@test.com' })

    setToken(jwt)

    expect(user.value).toEqual({
      id: 'user-456',
      email: 'bob@test.com',
      firstName: undefined,
      lastName: undefined,
      avatarUrl: undefined,
    })
  })

  it('setToken sets user to null on invalid JWT', () => {
    const { setToken, user, token } = useAuth()

    setToken('not-a-valid-jwt')

    expect(token.value).toBe('not-a-valid-jwt')
    expect(user.value).toBeNull()
  })

  it('clearToken resets all state', () => {
    const { setToken, clearToken, isAuthenticated, token, user, getToken } = useAuth()
    setToken(makeJwt({ sub: '1', email: 'test@test.com' }))
    expect(isAuthenticated.value).toBe(true)

    clearToken()

    expect(isAuthenticated.value).toBe(false)
    expect(token.value).toBeNull()
    expect(user.value).toBeNull()
    expect(getToken()).toBeNull()
  })

  it('shares state across multiple useAuth() calls (singleton)', () => {
    const auth1 = useAuth()
    const auth2 = useAuth()

    auth1.setToken(makeJwt({ sub: '1', email: 'shared@test.com' }))

    expect(auth2.isAuthenticated.value).toBe(true)
    expect(auth2.user.value?.email).toBe('shared@test.com')
  })
})
