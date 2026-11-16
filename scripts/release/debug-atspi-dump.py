import sys
import pyatspi

pid, name = int(sys.argv[1]), sys.argv[2]
desktop = pyatspi.Registry.getDesktop(0)
for app in desktop:
    try:
        if app is None or app.get_process_id() != pid:
            continue
    except Exception:
        continue
    print("app", app.name, "children", app.childCount)
    for window in app:
        try:
            print(" window", window.name, window.getRoleName(), window.getState().getStates())
        except Exception as error:
            print(" window error", error)
    stack = [app]
    while stack:
        node = stack.pop()
        try:
            if node.name == name:
                actions = []
                try:
                    action = node.queryAction()
                    actions = [action.getName(i) for i in range(action.nActions)]
                except Exception as error:
                    actions = [f"error {error}"]
                print(" node", node.getRoleName(), [pyatspi.stateToString(s) for s in node.getState().getStates()], actions)
            stack.extend(child for child in node if child is not None)
        except Exception:
            continue
