package io.subportal.android.ui.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import io.subportal.android.ui.screens.EnrollScreen
import io.subportal.android.ui.screens.ServerDetailScreen
import io.subportal.android.ui.screens.ServerListScreen

object Routes {
    const val SERVER_LIST = "server_list"
    const val ENROLL = "enroll"
    const val SERVER_DETAIL = "server_detail/{serverId}"

    fun serverDetail(serverId: String): String =
        "server_detail/${java.net.URLEncoder.encode(serverId, "UTF-8")}"
}

@Composable
fun SubportalNavGraph(navController: NavHostController) {
    NavHost(navController = navController, startDestination = Routes.SERVER_LIST) {
        composable(Routes.SERVER_LIST) {
            ServerListScreen(
                onEnrollClick = { navController.navigate(Routes.ENROLL) },
                onServerClick = { serverId ->
                    navController.navigate(Routes.serverDetail(serverId))
                },
            )
        }
        composable(Routes.ENROLL) {
            EnrollScreen(
                onEnrolled = { navController.popBackStack() },
                onBack = { navController.popBackStack() }
            )
        }
        composable(
            route = Routes.SERVER_DETAIL,
            arguments = listOf(navArgument("serverId") { type = NavType.StringType }),
        ) { backStackEntry ->
            val serverId = backStackEntry.arguments?.getString("serverId") ?: ""
            ServerDetailScreen(
                serverId = serverId,
                onBack = { navController.popBackStack() },
            )
        }
    }
}
