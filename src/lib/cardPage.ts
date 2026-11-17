// Opening a card's page at a run or a comment from elsewhere (GA-297: the Chat's Card tab). The page opens on its Comments
// tab; for a run it opens on Runs instead (TaskPage asks takeRunsTab as it opens), and the run's or the comment's row
// scrolls into sight with the focus (reveal).
import { go } from "../router";
import { reveal } from "./reveal";

/** The card whose page should open on its Runs tab, once. */
let runsFor: string | null = null;

/** The card's page (by the card's own id, not GA-12), on its Runs tab, at the run. */
export function openRunOnPage(taskId: string, runId: string) {
  runsFor = taskId;
  go({ page: "task", id: taskId });
  reveal(`run-${runId}`);
}
/** The card's page at the comment, on its Comments tab. */
export function openCommentOnPage(taskId: string, commentId: string) {
  go({ page: "task", id: taskId });
  reveal(`comment-${commentId}`);
}
/** Whether the card's page that opens now should show its Runs tab (it was asked for once). */
export function takeRunsTab(taskId: string): boolean {
  if (runsFor !== taskId) return false;
  runsFor = null;
  return true;
}
