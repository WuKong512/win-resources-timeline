export interface TimelineRefreshNoticeState {
  refreshing: boolean;
  refreshError: string;
}

/** Successful background refreshes stay silent; only a refresh error gets a notice. */
export function shouldShowTimelineRefreshNotice(state: TimelineRefreshNoticeState): boolean {
  return state.refreshError.trim().length > 0;
}

/** Background refreshes retain the last successful timeline while the request is in flight. */
export function shouldClearTimelineForLoad(background: boolean): boolean {
  return !background;
}
