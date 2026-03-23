const API_URL = import.meta.env.VITE_API_URL || ''

export class AuthError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'AuthError'
  }
}

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

export async function apiClient<T>(
  path: string,
  options?: RequestInit,
): Promise<T> {
  const response = await fetch(`${API_URL}${path}`, {
    ...options,
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  })

  if (response.status === 401) {
    throw new AuthError('Unauthorized')
  }

  if (!response.ok) {
    const text = await response.text()
    throw new ApiError(response.status, text || `Request failed with status ${response.status}`)
  }

  // Handle empty responses (204 No Content, 202 Accepted, or empty body)
  if (response.status === 204 || response.status === 202) {
    return undefined as T
  }

  const text = await response.text()
  if (!text) {
    return undefined as T
  }

  return JSON.parse(text)
}
