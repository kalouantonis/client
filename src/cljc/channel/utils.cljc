(ns channel.utils
  "General utilities useful for both the frontend and the backend."
  (:require #?(:clj  [clojure.core.async :as async :refer [go-loop]]
               :cljs [cljs.core.async :as async]))
  #?(:cljs (:require-macros [cljs.core.async.macros :refer [go-loop]])))

(defn any-nil?
  "Returns true if any items in collection `coll` are nil."
  [coll]
  (some (comp not nil?) coll))

(defn maybe
  "Returns a function that calls `f` with all parameters, given
  that one or more parameters are not nil."
  [f]
  (fn [& args]
    (when (any-nil? args)
      (apply f args))))

;;
;; Async
;;
;; TODO: Consider putting these in channel.async

(defmacro go-when
  "Runs a go block as long as `bindings` are truthy."
  [bindings & body]
  `(go-loop []
     (when-let ~bindings
       ~@body
       (recur))))

(defmacro go-while
  "Same as `while`, but is run within a `go` block."
  [test & body]
  `(go-loop []
     (when ~test
       ~@body
       (recur))))
